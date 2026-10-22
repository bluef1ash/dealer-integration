<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { api } from '../api'
import { showToast } from '../store'
import type { AdAccount, OceanAppStatus } from '../types'
import { fmtTime } from '../utils'

const apps = ref<OceanAppStatus[]>([])
const accounts = ref<AdAccount[]>([])
const hint = ref('')

const newName = ref('')
const newAppId = ref('')
const newSecret = ref('')
const adding = ref(false)
const authorizingId = ref<number | null>(null)

async function refresh(): Promise<void> {
  try {
    apps.value = await api.listOceanApps()
    accounts.value = await api.listAdAccounts()
  } catch (e) {
    showToast(`读取巨量应用失败：${e}`)
  }
}

/** token 已过期（调度器会在过期前 5 分钟自动刷新，过期仅发生在应用未运行期间） */
function isExpired(a: OceanAppStatus): boolean {
  return a.token_expires_at !== null && a.token_expires_at < Math.floor(Date.now() / 1000)
}

async function addApp(): Promise<void> {
  if (!newName.value.trim() || !newAppId.value.trim() || !newSecret.value.trim()) {
    hint.value = '应用名称、App ID 与 App Secret 均不能为空'
    return
  }
  adding.value = true
  try {
    await api.addOceanApp(newName.value.trim(), newAppId.value.trim(), newSecret.value.trim())
    newName.value = ''
    newAppId.value = ''
    newSecret.value = ''
    hint.value = '应用已添加，点「去授权」在系统浏览器完成 OAuth'
    await refresh()
  } catch (e) {
    hint.value = `添加失败：${e}`
  } finally {
    adding.value = false
  }
}

async function removeApp(a: OceanAppStatus): Promise<void> {
  if (!window.confirm(`删除应用「${a.name}」？其授权 token 将一并清除，已拉取的报表数据保留。`)) {
    return
  }
  try {
    await api.removeOceanApp(a.id)
    hint.value = `应用「${a.name}」已删除`
    await refresh()
  } catch (e) {
    hint.value = `删除失败：${e}`
  }
}

async function authorize(a: OceanAppStatus): Promise<void> {
  authorizingId.value = a.id
  hint.value = `「${a.name}」已打开系统浏览器，请在巨量引擎授权页完成操作…`
  try {
    const result = await api.startOceanAuth(a.id)
    hint.value = `「${a.name}」授权成功，同步到 ${result.accounts.length} 个广告账户`
    await refresh()
  } catch (e) {
    hint.value = `「${a.name}」授权失败：${e}`
  } finally {
    authorizingId.value = null
  }
}

onMounted(refresh)

const accountCount = computed(() => accounts.value.length)
const accountSummary = computed(() =>
  accounts.value.length
    ? accounts.value.map((a) => a.alias || a.id).join('、')
    : '尚无，完成任一应用授权后自动进入映射表',
)
</script>

<template>
  <section class="settings-pane" role="tabpanel">
    <section class="panel">
      <div class="panel-head">
        <div>
          <h2>巨量引擎应用</h2>
          <p class="sub">商业开放平台的应用凭证，一个应用可授权多个广告账户；类似巨懂车多账号，各自独立授权</p>
        </div>
        <span class="pill" :class="apps.length ? 'pill-ok' : 'pill-warn'">
          {{ apps.length ? `${apps.length} 个应用` : '未配置' }}
        </span>
      </div>

      <div class="mapping-wrap">
        <table class="ds-table store-table">
          <thead>
            <tr>
              <th>应用 / App ID</th>
              <th>授权</th>
              <th>到期时间</th>
              <th class="num-col">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="a in apps" :key="a.id">
              <td>
                {{ a.name }}
                <small class="account">{{ a.app_id }}</small>
              </td>
              <td>
                <span v-if="!a.has_token" class="pill pill-muted">未授权</span>
                <span v-else-if="isExpired(a)" class="pill pill-warn">
                  <span class="dot warn"></span>已过期
                </span>
                <span v-else class="pill pill-ok"><span class="dot ok"></span>有效</span>
              </td>
              <td><span class="last-time">{{ a.token_expires_at ? fmtTime(a.token_expires_at) : '—' }}</span></td>
              <td>
                <div class="row-actions">
                  <button class="btn" :disabled="authorizingId === a.id" @click="authorize(a)">
                    {{ authorizingId === a.id ? '授权中…' : a.has_token ? '重新授权' : '去授权' }}
                  </button>
                  <button class="btn danger" @click="removeApp(a)">删除</button>
                </div>
              </td>
            </tr>
            <tr v-if="!apps.length">
              <td colspan="4" class="empty-row">还没有应用，用下方表单添加第一个。</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="add-store">
        <input v-model="newName" class="input" placeholder="应用名称，如：主应用" aria-label="应用名称" />
        <input
          v-model="newAppId"
          class="input"
          placeholder="App ID（数字）"
          aria-label="App ID"
          autocomplete="off"
          spellcheck="false"
        />
        <input
          v-model="newSecret"
          class="input span-2"
          type="password"
          placeholder="App Secret"
          aria-label="App Secret"
          autocomplete="new-password"
          spellcheck="false"
        />
        <button class="btn" :disabled="adding" @click="addApp">{{ adding ? '添加中…' : '添加应用' }}</button>
      </div>
      <p class="form-hint">
        凭证存本地 SQLite（ocean_apps 表），token 存系统凭证库；OAuth 回调本机 127.0.0.1:1720。
        Secret 保存后不再回显，需要更换时删除应用重新添加——同一 App ID 重录不影响已授权 token。
      </p>
      <p v-if="hint" class="banner">{{ hint }}</p>
    </section>

    <section class="panel panel-note">
      <h2>应用与账户的关系</h2>
      <ul class="note-list">
        <li><b>多应用</b>：每个应用对应商业开放平台的一组 App ID / Secret，可分别授权不同主体。</li>
        <li><b>账户归属</b>：授权返回的广告账户归属该应用；同一账户在其他应用下再次授权时改判归属。</li>
        <li><b>报表拉取</b>：调度器逐应用刷新 token（过期前 5 分钟）并拉取其名下账户的日报，未授权的应用自动跳过。</li>
        <li><b>已授权账户</b>（{{ accountCount }}）：{{ accountSummary }}</li>
      </ul>
    </section>
  </section>
</template>
