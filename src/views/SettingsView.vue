<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { onMounted, onUnmounted, ref } from 'vue'
import { api } from '../api'
import DataState from '../components/DataState.vue'
import OceanCredentialsCard from '../components/OceanCredentialsCard.vue'
import type { AdAccount, FetchState, LoginEvent, SnapshotOutcome, StoreStatus } from '../types'
import { fmtTime } from '../utils'

const state = ref<FetchState>('loading')
const statuses = ref<StoreStatus[]>([])
const accounts = ref<AdAccount[]>([])
/** store_id → 已映射账户 ID 集合 */
const mappings = ref<Map<number, Set<string>>>(new Map())
const loginHint = ref('')
const humanHint = ref('')
const snapshotResults = ref<SnapshotOutcome[]>([])

// 添加门店表单
const newName = ref('')
const newAccount = ref('')
const newPassword = ref('')
const adding = ref(false)

const snapshotHour = ref(23)
const schedEnabled = ref(true)
const schedInterval = ref(60)
const INTERVAL_OPTIONS = [
  { value: 30, label: '每 30 分钟' },
  { value: 60, label: '每 1 小时' },
  { value: 120, label: '每 2 小时' },
  { value: 180, label: '每 3 小时' },
]
const pullingId = ref<number | null>(null)

let unlisteners: Array<() => void> = []

async function refresh(): Promise<void> {
  state.value = 'loading'
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
    state.value = 'ready'
  } catch (e) {
    state.value = 'error'
    loadError.value = String(e)
  }
}

const loadError = ref('')

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

async function authorizeOcean(): Promise<void> {
  loginHint.value = '已打开系统浏览器完成巨量引擎授权…'
  try {
    const result = await api.startOceanAuth()
    loginHint.value = `授权成功，同步到 ${result.accounts.length} 个广告账户`
    await refresh()
  } catch (e) {
    loginHint.value = `授权失败：${e}`
  }
}

async function toggleMapping(s: StoreStatus, accountId: string, checked: boolean): Promise<void> {
  await api.setStoreMapping(s.store.id, accountId, checked)
  mappings.value.get(s.store.id)?.[checked ? 'add' : 'delete'](accountId)
}

async function saveHour(): Promise<void> {
  await api.setSnapshotHour(snapshotHour.value)
  loginHint.value = `对账快照时刻已设为每天 ${snapshotHour.value}:00`
}

const snapRunning = ref(false)
async function saveScheduler(): Promise<void> {
  try {
    await api.setSchedulerConfig(schedEnabled.value, schedInterval.value)
    loginHint.value = schedEnabled.value
      ? `自动拉取已开启，${schedInterval.value} 分钟一次（下一轮生效）`
      : '自动拉取已关闭，仅手动拉取'
  } catch (e) {
    loginHint.value = `保存失败：${e}`
  }
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
</script>

<template>
  <section class="toolbar">
    <h1>设置</h1>
  </section>

  <p v-if="loginHint" class="banner">{{ loginHint }}</p>
  <p v-if="humanHint" class="banner warn">⚠ {{ humanHint }}</p>

  <OceanCredentialsCard />

  <section class="card">
    <h2>门店与巨懂车账号</h2>
    <DataState :state="state">
      <template v-if="state === 'error'">
        <p class="error">读取失败：{{ loadError }}</p>
        <button type="button" class="ghost" @click="refresh">重试</button>
      </template>
      <template v-else>
        <div class="tblwrap">
          <table>
            <thead>
              <tr><th>门店</th><th>巨懂车账号</th><th>会话</th><th>最近成功采集</th><th>回填</th><th>操作</th></tr>
            </thead>
            <tbody>
              <tr v-for="s in statuses" :key="s.store.id">
                <td>{{ s.store.name }}</td>
                <td>{{ s.store.account }}</td>
                <td>
                  <span
                    v-if="!s.has_session"
                    class="bad"
                  >未登录</span>
                  <span
                    v-else-if="s.last_error"
                    class="bad"
                    :title="s.last_error"
                  >拉取失败</span>
                  <span v-else class="ok">有效</span>
                </td>
                <td>{{ fmtTime(s.last_success_at) }}</td>
                <td>{{ s.backfilled ? '已完成' : '待回填' }}</td>
                <td class="ops">
                  <button type="button" class="ghost" @click="startLogin(s)">
                    {{ s.has_session ? '重新登录' : '登录' }}
                  </button>
                  <button type="button" class="ghost" :disabled="pullingId === s.store.id" @click="pullOne(s)">
                    {{ pullingId === s.store.id ? '拉取中…' : '拉取' }}
                  </button>
                  <button type="button" class="ghost danger" @click="removeStore(s)">删除</button>
                </td>
              </tr>
              <tr v-if="statuses.length === 0">
                <td colspan="6" class="none">还没有门店，用下方表单添加第一家。</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="add-form">
          <input v-model="newName" type="text" placeholder="门店名称，如：城东店" />
          <input v-model="newAccount" type="text" placeholder="巨懂车账号（手机号）" />
          <input v-model="newPassword" type="password" placeholder="密码（可选，供自动重登）" autocomplete="new-password" />
          <button type="button" :disabled="adding" @click="addStore">{{ adding ? '添加中…' : '+ 添加门店' }}</button>
        </div>
        <p class="hint">
          密码仅存系统凭证库（Windows 凭据管理器 / macOS 钥匙串），供会话失效自动重登；不填则失效时需人工登录。
          登录在应用内嵌窗口完成，支持账号密码或短信验证码，滑块需人工拖动。
        </p>

        <template v-if="accounts.length > 0">
          <h2 class="sub">门店 ↔ 巨量广告账户映射</h2>
          <div class="tblwrap">
            <table>
              <thead>
                <tr>
                  <th>门店</th>
                  <th v-for="a in accounts" :key="a.id">{{ a.alias || a.id }}</th>
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
              </tbody>
            </table>
          </div>
        </template>

        <div class="ocean-ops">
          <button type="button" class="ghost" @click="authorizeOcean">+ 授权巨量广告账户</button>
          <span class="hint">已在开放平台配置 App ID / Secret 后可用；授权在系统浏览器完成，回调本机 127.0.0.1:1720。</span>
        </div>
      </template>
    </DataState>
  </section>

  <section class="card">
    <h2>定时拉取</h2>
    <div class="snap-row">
      <label>
        <input v-model="schedEnabled" type="checkbox" />
        自动拉取
      </label>
      <select v-model.number="schedInterval" class="hour" :disabled="!schedEnabled">
        <option v-for="o in INTERVAL_OPTIONS" :key="o.value" :value="o.value">{{ o.label }}</option>
      </select>
      <button type="button" class="ghost" @click="saveScheduler">保存</button>
    </div>
    <p class="hint">
      开启时应用运行期间按所选间隔自动拉取线索与报表（新门店首轮回填 90 天，之后每轮重拉近 7 天）；
      关闭后仅手动拉取，每日对账快照也一并停用。修改后下一轮生效。
    </p>
  </section>

  <section class="card">
    <h2>对账快照</h2>
    <div class="snap-row">
      <label>每日 <input v-model.number="snapshotHour" type="number" min="0" max="23" class="hour" /> 点执行</label>
      <button type="button" class="ghost" @click="saveHour">保存时刻</button>
      <button type="button" class="ghost" :disabled="snapRunning" @click="snapshotNow">
        {{ snapRunning ? '对账中…' : '立即对账' }}
      </button>
    </div>
    <div v-if="snapshotResults.length" class="tblwrap">
      <table>
        <thead><tr><th>门店</th><th>结果</th><th>快照行数</th></tr></thead>
        <tbody>
          <tr v-for="r in snapshotResults" :key="r.store_id">
            <td>{{ statuses.find((s) => s.store.id === r.store_id)?.store.name ?? r.store_id }}</td>
            <td><span :class="r.ok ? 'ok' : 'bad'">{{ r.ok ? '完成' : '失败' }}</span>{{ r.ok ? '' : `：${r.error}` }}</td>
            <td>{{ r.seen }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <p class="hint">对账每日自动触发一次导出并合并，纠正增量拉取的字段漂移；一天只产生一条导出记录。</p>
  </section>
</template>

<style scoped>
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; }
.toolbar h1 { font-size: 22px; }
.banner {
  background: #eef2ff; color: #3730a3; border-radius: 10px;
  padding: 10px 16px; font-size: 13px; margin-bottom: 16px;
}
.banner.warn { background: #fef3c7; color: #92400e; }
.card {
  background: #fff; border: 1px solid #e7e5e4; border-radius: 16px;
  padding: 16px 20px; margin-bottom: 20px;
}
.card h2 { font-size: 16px; margin-bottom: 10px; }
.card h2.sub { margin-top: 20px; }
.tblwrap { border: 1px solid #f0efee; border-radius: 12px; overflow-x: auto; }
table { width: 100%; border-collapse: collapse; font-size: 13px; }
th, td { text-align: left; padding: 8px 10px; border-bottom: 1px solid #f0efee; color: #44403c; }
th { color: #a8a29e; font-weight: 500; font-size: 12px; }
.center { text-align: center; }
.ok { color: #059669; }
.bad { color: #92400e; background: #fef3c7; border-radius: 6px; padding: 2px 8px; font-size: 12px; }
.ops { display: flex; gap: 6px; }
.ghost {
  border: 1px solid #4f46e5; color: #4f46e5; background: none;
  border-radius: 10px; padding: 5px 12px; font-size: 12px; cursor: pointer; white-space: nowrap;
}
.ghost:disabled { color: #a8a29e; border-color: #e7e5e4; cursor: default; }
.ghost.danger { border-color: #dc2626; color: #dc2626; }
.add-form { display: flex; gap: 10px; margin-top: 14px; flex-wrap: wrap; }
.add-form input {
  flex: 1; min-width: 140px; border: 1px solid #d6d3d1; border-radius: 10px;
  padding: 8px 12px; font-size: 13px;
}
.add-form button {
  border: 1px solid #4f46e5; color: #fff; background: #4f46e5;
  border-radius: 10px; padding: 8px 16px; font-size: 13px; cursor: pointer;
}
.add-form button:disabled { opacity: 0.6; cursor: default; }
.hint { font-size: 12px; color: #a8a29e; margin-top: 10px; }
.error { font-size: 13px; color: #991b1b; margin-bottom: 10px; }
.none { color: #a8a29e; text-align: center; padding: 16px 0; }
.ocean-ops { margin-top: 16px; display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.snap-row { display: flex; gap: 12px; align-items: center; margin-bottom: 12px; flex-wrap: wrap; }
.snap-row label { font-size: 13px; color: #57534e; display: flex; align-items: center; gap: 6px; }
.snap-row input[type='checkbox'] { width: 16px; height: 16px; }
.hour { width: 60px; border: 1px solid #d6d3d1; border-radius: 8px; padding: 6px 8px; font-size: 13px; }
</style>
