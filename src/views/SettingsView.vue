<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { api } from '../api'
import OceanAppsPanel from '../components/OceanAppsPanel.vue'
import { showToast } from '../store'
import type { AdAccount, LoginEvent, SnapshotOutcome, StoreStatus } from '../types'
import { fmtTime } from '../utils'

type SettingsTab = 'stores' | 'mapping' | 'ocean' | 'schedule' | 'data'
const TABS: Array<{ key: SettingsTab; label: string }> = [
  { key: 'stores', label: '门店与账号' },
  { key: 'mapping', label: '广告账户映射' },
  { key: 'ocean', label: '巨量引擎应用' },
  { key: 'schedule', label: '采集与对账' },
  { key: 'data', label: '数据与凭证' },
]
const tab = ref<SettingsTab>('stores')

const statuses = ref<StoreStatus[]>([])
const accounts = ref<AdAccount[]>([])
const mappings = ref<Map<number, Set<string>>>(new Map())
const loginHint = ref('')
const humanHint = ref('')
const snapshotResults = ref<SnapshotOutcome[]>([])
const snapRunning = ref(false)
const pullingId = ref<number | null>(null)

const newName = ref('')
const newAccount = ref('')
const newPassword = ref('')
const adding = ref(false)

const schedEnabled = ref(true)
const schedInterval = ref(60)
const snapshotHour = ref(23)

const unlisteners: Array<() => void> = []

async function refresh(): Promise<void> {
  try {
    statuses.value = await api.getCollectionStatus()
    accounts.value = await api.listAdAccounts()
    const map = new Map<number, Set<string>>()
    for (const s of statuses.value) {
      map.set(s.store.id, new Set(await api.getStoreAdAccounts(s.store.id)))
    }
    mappings.value = map
    snapshotHour.value = await api.getSnapshotHour()
    const sched = await api.getSchedulerConfig()
    schedEnabled.value = sched.enabled
    schedInterval.value = sched.interval_minutes
  } catch (e) {
    showToast(`读取设置失败：${e}`)
  }
}

async function addStore(): Promise<void> {
  if (!newName.value.trim() || !newAccount.value.trim()) {
    loginHint.value = '门店名称与账号标识不能为空'
    return
  }
  adding.value = true
  try {
    await api.addStore(
      newName.value.trim(),
      newAccount.value.trim(),
      newPassword.value.trim() || null,
    )
    newName.value = ''
    newAccount.value = ''
    newPassword.value = ''
    loginHint.value = '门店已添加，点「登录」完成巨懂车授权'
    await refresh()
  } catch (e) {
    loginHint.value = `添加失败：${e}`
  } finally {
    adding.value = false
  }
}

async function removeStore(s: StoreStatus): Promise<void> {
  if (!window.confirm(`删除门店「${s.store.name}」？其登录凭证将一并清除。`)) {
    return
  }
  await api.removeStore(s.store.id)
  await refresh()
}

async function startLogin(s: StoreStatus): Promise<void> {
  loginHint.value = `已打开登录窗口（${s.store.name}），请在窗口内完成登录…`
  humanHint.value = ''
  try {
    await api.startLogin(s.store.id)
  } catch (e) {
    loginHint.value = `发起登录失败：${e}`
  }
}

async function pullOne(s: StoreStatus): Promise<void> {
  pullingId.value = s.store.id
  try {
    const outcomes = await api.pullNow(s.store.id)
    const o = outcomes[0]
    loginHint.value = o?.ok
      ? `「${s.store.name}」入库 ${o.ingested} 条`
      : `「${s.store.name}」拉取失败：${o?.error ?? ''}`
    await refresh()
  } finally {
    pullingId.value = null
  }
}

async function toggleMapping(s: StoreStatus, accountId: string, checked: boolean): Promise<void> {
  await api.setStoreMapping(s.store.id, accountId, checked)
  mappings.value.get(s.store.id)?.[checked ? 'add' : 'delete'](accountId)
}

async function saveScheduler(): Promise<void> {
  // v-model.number 清空输入时得到空串，先做前端校验避免把非数字发给后端
  if (
    !Number.isInteger(schedInterval.value) ||
    schedInterval.value < 15 ||
    schedInterval.value > 1440
  ) {
    loginHint.value = '拉取间隔须为 15-1440 之间的整数分钟'
    return
  }
  try {
    await api.setSchedulerConfig(schedEnabled.value, schedInterval.value)
    loginHint.value = schedEnabled.value
      ? `自动拉取已开启，${schedInterval.value} 分钟一次（下一轮生效）`
      : '自动拉取已关闭，仅手动拉取'
  } catch (e) {
    loginHint.value = `保存失败：${e}`
  }
}

async function saveHour(): Promise<void> {
  await api.setSnapshotHour(snapshotHour.value)
  loginHint.value = `对账快照时刻已设为每天 ${snapshotHour.value}:00`
}

async function snapshotNow(): Promise<void> {
  snapRunning.value = true
  snapshotResults.value = []
  try {
    snapshotResults.value = await api.runSnapshot(null)
    loginHint.value = '对账快照执行完毕'
  } catch (e) {
    loginHint.value = `快照失败：${e}`
  } finally {
    snapRunning.value = false
  }
}

onMounted(async () => {
  await refresh()
  unlisteners.push(
    await listen<LoginEvent>('login://result', async (e) => {
      const { store_id, ok, message } = e.payload
      const name =
        statuses.value.find((s) => s.store.id === store_id)?.store.name ?? `门店${store_id}`
      loginHint.value = ok
        ? `「${name}」${message}，会话已保存`
        : `「${name}」登录未完成：${message}`
      if (ok) {
        await refresh()
      }
    }),
    await listen<{ store_id: number; ok: boolean; error: string }>('login://pulled', async (e) => {
      const name =
        statuses.value.find((s) => s.store.id === e.payload.store_id)?.store.name ??
        `门店${e.payload.store_id}`
      loginHint.value = e.payload.ok
        ? `「${name}」登录后自动拉取完成`
        : `「${name}」登录后自动拉取失败：${e.payload.error}`
      await refresh()
    }),
    await listen<{ store_id: number; reason: string }>('login://needs-human', async (e) => {
      const name =
        statuses.value.find((s) => s.store.id === e.payload.store_id)?.store.name ??
        `门店${e.payload.store_id}`
      humanHint.value = `「${name}」${e.payload.reason}：请到设置页点「登录」完成人工验证。`
    }),
  )
})

onUnmounted(() => {
  for (const off of unlisteners) {
    off()
  }
})

const storeCount = computed(() => statuses.value.length)
const accountCount = computed(() => accounts.value.length)
</script>

<template>
  <div class="settings-wrap">
    <nav class="settings-tabs" role="tablist" aria-label="设置分区">
      <button
        v-for="t in TABS"
        :key="t.key"
        class="settings-tab"
        :class="{ 'is-active': tab === t.key }"
        role="tab"
        :aria-selected="tab === t.key"
        @click="tab = t.key"
      >
        <span>{{ t.label }}</span>
        <span v-if="t.key === 'stores'" class="sn-count">{{ storeCount }}</span>
        <span v-if="t.key === 'mapping'" class="sn-count">{{ accountCount }}</span>
      </button>
    </nav>

    <p v-if="loginHint" class="banner">{{ loginHint }}</p>
    <p v-if="humanHint" class="banner warn">⚠ {{ humanHint }}</p>

    <!-- ── 门店与账号 ── -->
    <section v-show="tab === 'stores'" class="settings-pane" role="tabpanel">
      <section class="panel">
        <div class="panel-head">
          <div>
            <h2>门店与巨懂车账号</h2>
            <p class="sub">每店一个巨懂车账号，会话失效时可自动或人工重登</p>
          </div>
          <span class="pill">{{ storeCount }} 家</span>
        </div>
        <div class="mapping-wrap">
          <table class="ds-table store-table">
            <thead>
              <tr>
                <th>门店 / 账号</th>
                <th>会话</th>
                <th>最近采集</th>
                <th class="num-col">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="s in statuses" :key="s.store.id">
                <td>
                  {{ s.store.name }}
                  <small class="account">{{ s.store.account }}</small>
                </td>
                <td>
                  <span v-if="!s.has_session" class="pill pill-muted">未登录</span>
                  <span v-else-if="s.last_error" class="pill pill-warn" :title="s.last_error">
                    <span class="dot warn"></span>拉取失败
                  </span>
                  <span v-else class="pill pill-ok"><span class="dot ok"></span>有效</span>
                </td>
                <td><span class="last-time">{{ fmtTime(s.last_success_at) }}</span></td>
                <td>
                  <div class="row-actions">
                    <button class="btn" @click="startLogin(s)">{{ s.has_session ? '重新登录' : '登录' }}</button>
                    <button class="btn" :disabled="pullingId === s.store.id" @click="pullOne(s)">
                      {{ pullingId === s.store.id ? '拉取中…' : '拉取' }}
                    </button>
                    <button class="btn danger" @click="removeStore(s)">删除</button>
                  </div>
                </td>
              </tr>
              <tr v-if="!statuses.length">
                <td colspan="4" class="empty-row">还没有门店，用下方表单添加第一家。</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="add-store">
          <input v-model="newName" class="input" placeholder="门店名称，如：城北店" aria-label="门店名称" />
          <input v-model="newAccount" class="input" placeholder="巨懂车账号（手机号）" aria-label="巨懂车账号" />
          <input
            v-model="newPassword"
            class="input span-2"
            type="password"
            placeholder="密码（可选，供自动重登）"
            aria-label="密码"
            autocomplete="new-password"
          />
          <button class="btn" :disabled="adding" @click="addStore">{{ adding ? '添加中…' : '添加门店' }}</button>
        </div>
        <p class="form-hint">
          密码存本地 SQLite（随应用数据目录的数据库文件走），仅用于会话失效时自动重登；
          登录在应用内嵌窗口完成，支持账号密码或短信验证码，滑块需人工拖动。
        </p>
      </section>
    </section>

    <!-- ── 广告账户映射 ── -->
    <section v-show="tab === 'mapping'" class="settings-pane" role="tabpanel">
      <section class="panel">
        <div class="panel-head">
          <div>
            <h2>门店 ↔ 广告账户映射</h2>
            <p class="sub">多对多映射决定广告消耗如何归因到门店</p>
          </div>
        </div>
        <div class="mapping-wrap">
          <table class="ds-table mapping-table">
            <thead>
              <tr>
                <th>门店</th>
                <th v-for="a in accounts" :key="a.id">
                  {{ a.alias || a.id }}
                  <small v-if="a.app_name" class="map-app">{{ a.app_name }}</small>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="s in statuses" :key="s.store.id">
                <td>{{ s.store.name }}</td>
                <td v-for="a in accounts" :key="a.id" class="center">
                  <input
                    type="checkbox"
                    :checked="mappings.get(s.store.id)?.has(a.id)"
                    @change="toggleMapping(s, a.id, ($event.target as HTMLInputElement).checked)"
                  />
                </td>
              </tr>
              <tr v-if="!accounts.length || !statuses.length">
                <td :colspan="accounts.length + 1" class="empty-row">
                  {{ accounts.length ? '先添加门店' : '尚无广告账户：先在「巨量引擎应用」完成 OAuth 授权' }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="form-hint">勾选后立即保存；未映射的门店只统计线索，不计算广告消耗。</p>
      </section>
    </section>

    <!-- ── 巨量引擎应用（多应用面板，独立组件） ── -->
    <section v-show="tab === 'ocean'" class="settings-pane" role="tabpanel">
      <OceanAppsPanel />
    </section>

    <!-- ── 采集与对账 ── -->
    <section v-show="tab === 'schedule'" class="settings-pane" role="tabpanel">
      <section class="panel">
        <div class="panel-head">
          <div>
            <h2>采集调度</h2>
            <p class="sub">应用运行期间按间隔自动拉取</p>
          </div>
        </div>
        <label class="switch-row">
          <span>
            <b>自动拉取</b>
            <small>关闭后仅手动拉取，每日对账也停用</small>
          </span>
          <input v-model="schedEnabled" type="checkbox" />
          <span class="switch" aria-hidden="true"></span>
        </label>
        <div class="field-row">
          <label class="field">
            <span>拉取间隔（分钟）</span>
            <input
              v-model.number="schedInterval"
              class="input"
              type="number"
              min="15"
              max="1440"
              step="1"
              placeholder="60"
              :disabled="!schedEnabled"
            />
          </label>
        </div>
        <p class="form-hint">间隔按分钟计算，可填 15-1440 的整数（如 30 = 每半小时、1440 = 每天）。新门店首轮回填 90 天，之后每轮重拉最近 7 天，覆盖线索状态变更。修改后下一轮生效。</p>
        <div class="form-actions">
          <button class="btn btn-sm" @click="saveScheduler">保存设置</button>
        </div>
      </section>

      <section class="panel">
        <div class="panel-head">
          <div>
            <h2>对账快照</h2>
            <p class="sub">每日导出一次巨懂车表格，纠正增量字段漂移</p>
          </div>
        </div>
        <div class="field-row">
          <label class="field">
            <span>每日执行时刻（0-23 点）</span>
            <input v-model.number="snapshotHour" class="input" type="number" min="0" max="23" step="1" />
          </label>
          <div class="field">
            <span>合并口径</span>
            <p class="readout">按飞鱼线索 ID 合并</p>
          </div>
        </div>
        <div class="form-actions">
          <button class="btn btn-sm" @click="saveHour">保存时刻</button>
          <button class="btn btn-sm" :disabled="snapRunning" @click="snapshotNow">
            {{ snapRunning ? '对账中…' : '立即对账' }}
          </button>
        </div>
        <div v-if="snapshotResults.length" class="mapping-wrap">
          <table class="ds-table store-table">
            <thead>
              <tr><th>门店</th><th>结果</th><th class="num-col">快照行数</th></tr>
            </thead>
            <tbody>
              <tr v-for="r in snapshotResults" :key="r.store_id">
                <td>{{ statuses.find((s) => s.store.id === r.store_id)?.store.name ?? r.store_id }}</td>
                <td>
                  <span class="pill" :class="r.ok ? 'pill-ok' : 'pill-warn'">{{ r.ok ? '完成' : '失败' }}</span>
                  {{ r.ok ? '' : `：${r.error}` }}
                </td>
                <td class="num-col">{{ r.seen }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </section>

    <!-- ── 数据与凭证 ── -->
    <section v-show="tab === 'data'" class="settings-pane" role="tabpanel">
      <section class="panel panel-note">
        <h2>数据与凭证</h2>
        <ul class="note-list">
          <li><b>业务数据</b>：存本地 SQLite，位于系统应用数据目录。</li>
          <li><b>登录凭证</b>：巨懂车会话、巨量 OAuth token 存系统凭证库；巨懂车密码存本地 SQLite（stores 表）。</li>
          <li><b>巨量引擎应用</b>：多应用配置存本地 SQLite（ocean_apps 表），Secret 不回显。</li>
          <li><b>线索 ID</b>：以列表接口为主键，快照按飞鱼线索 ID 找回后合并。</li>
          <li><b>分发</b>：为手动安装包，无自动更新与多租户。</li>
        </ul>
      </section>
    </section>
  </div>
</template>

<style scoped>
.map-app {
  display: block;
  font-size: 10.5px;
  font-weight: 400;
  color: var(--muted);
}
</style>
