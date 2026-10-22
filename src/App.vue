<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { api } from './api'
import { showToast, ui, type ViewKey } from './store'
import type { PullOutcome, Store, StoreStatus } from './types'
import { fmtTime } from './utils'
import BidView from './views/BidView.vue'
import DashboardView from './views/DashboardView.vue'
import LeadsView from './views/LeadsView.vue'
import SettingsView from './views/SettingsView.vue'

const NAV: Array<{ group: string; items: Array<{ key: ViewKey; label: string; icon: string }> }> = [
  {
    group: '运营',
    items: [
      {
        key: 'dashboard',
        label: '总览看板',
        icon: 'M3.5 3.5h7v7h-7zM13.5 3.5h7v7h-7zM3.5 13.5h7v7h-7zM13.5 13.5h7v7h-7z',
      },
      {
        key: 'bid',
        label: '竞价投放',
        icon: 'M4 19V9M9.5 19V5M15 19v-6M20.5 19V8',
      },
      {
        key: 'leads',
        label: '线索明细',
        icon: 'M4 6.5h16M4 12h16M4 17.5h9',
      },
    ],
  },
  {
    group: '配置',
    items: [{ key: 'settings', label: '设置', icon: 'M4 7h9M17 7h3M4 17h3M10 17h10' }],
  },
]

const PAGE_HEAD: Record<ViewKey, { title: string; sub: string }> = {
  dashboard: { title: '总览看板', sub: '线索量、消耗与单条线索成本总览' },
  bid: { title: '竞价投放', sub: '巨量引擎账户与计划层级的投放表现' },
  leads: { title: '线索明细', sub: '按时间、门店与状态查询线索' },
  settings: { title: '设置', sub: '门店账号、授权与采集调度' },
}

const stores = ref<Store[]>([])
const statuses = ref<StoreStatus[]>([])
const pulling = ref(false)
const lastPullAt = ref<number | null>(null)
const unlisteners: Array<() => void> = []

const win = getCurrentWindow()

const healthy = computed(() => statuses.value.every((s) => !s.last_error))
const healthText = computed(() => {
  if (!statuses.value.length) {
    return '尚无门店，先到设置页添加'
  }
  const bad = statuses.value.filter((s) => s.last_error)
  if (!bad.length) {
    return '全部门店采集正常'
  }
  return `${bad.map((s) => s.store.name).join('、')} 拉取失败 · ${bad.length} 项待处理`
})
const onlineStores = computed(() => statuses.value.filter((s) => s.has_session).length)
const pageHead = computed(() => {
  const scope =
    ui.storeId === null ? '全部门店' : (stores.value.find((s) => s.id === ui.storeId)?.name ?? '')
  return { ...PAGE_HEAD[ui.view], sub: `${scope} · ${PAGE_HEAD[ui.view].sub}` }
})
const syncText = computed(() => {
  if (pulling.value) {
    return '同步中…'
  }
  return lastPullAt.value ? `${fmtTime(lastPullAt.value)} 已同步` : '尚未同步'
})

async function refreshStatus(): Promise<void> {
  try {
    statuses.value = await api.getCollectionStatus()
  } catch (e) {
    showToast(`读取采集状态失败：${e}`)
  }
}

async function pullNow(): Promise<void> {
  pulling.value = true
  try {
    const outcomes: PullOutcome[] = await api.pullNow(ui.storeId)
    const ok = outcomes.filter((o) => o.ok)
    lastPullAt.value = Math.floor(Date.now() / 1000)
    showToast(
      ok.length
        ? `已拉取 ${ok.length} 家门店、入库 ${ok.reduce((n, o) => n + o.ingested, 0)} 条线索`
        : `拉取失败：${outcomes[0]?.error ?? '无门店'}`,
    )
    await refreshStatus()
  } catch (e) {
    showToast(`拉取失败：${e}`)
  } finally {
    pulling.value = false
  }
}

async function toggleMaximize(): Promise<void> {
  await win.toggleMaximize()
}

onMounted(async () => {
  stores.value = await api.listStores().catch(() => [])
  await refreshStatus()
  unlisteners.push(
    await listen('login://pulled', () => refreshStatus()),
    await listen<{ store_id: number; reason: string }>('login://needs-human', (e) => {
      const name =
        statuses.value.find((s) => s.store.id === e.payload.store_id)?.store.name ??
        `门店${e.payload.store_id}`
      showToast(`「${name}」${e.payload.reason}`)
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
  <header class="titlebar" data-tauri-drag-region>
    <button
      class="tb-icon"
      :aria-pressed="ui.rail"
      aria-label="收起导航"
      title="收起导航"
      @click="ui.rail = !ui.rail"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
        <rect x="3.5" y="4.5" width="17" height="15" rx="2" />
        <path d="M9.5 4.5v15" />
      </svg>
    </button>
    <div class="tb-brand" data-tauri-drag-region>
      <div class="brand-mark" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round">
          <path d="M4 19 10 5M13 19 19 5" />
        </svg>
      </div>
      <b>经销商运营台</b>
      <span class="tb-version">v0.1.0</span>
    </div>
    <div class="tb-status" data-tauri-drag-region>
      <span class="dot" :class="healthy ? 'ok' : 'warn'"></span>
      <span>{{ healthText }}</span>
    </div>
    <div class="win-controls">
      <button class="win-btn" aria-label="最小化窗口" @click="win.minimize()">
        <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2"><path d="M2 6h8" /></svg>
      </button>
      <button class="win-btn" aria-label="最大化窗口" @click="toggleMaximize">
        <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2">
          <rect x="2.5" y="2.5" width="7" height="7" rx="1" />
        </svg>
      </button>
      <button class="win-btn close" aria-label="关闭窗口" @click="win.close()">
        <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2"><path d="M3 3l6 6M9 3l-6 6" /></svg>
      </button>
    </div>
  </header>

  <div class="app" :class="{ rail: ui.rail }">
    <aside class="sidebar">
      <nav class="nav" aria-label="主导航">
        <div v-for="g in NAV" :key="g.group" class="nav-group">
          <p class="nav-label">{{ g.group }}</p>
          <button
            v-for="item in g.items"
            :key="item.key"
            class="nav-item"
            :class="{ 'is-active': ui.view === item.key }"
            :title="item.label"
            @click="ui.view = item.key"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
              <path :d="item.icon" />
            </svg>
            <span>{{ item.label }}</span>
          </button>
        </div>
      </nav>

      <div class="sidebar-foot">
        <div class="engine">
          <div class="engine-head">
            <span class="dot" :class="healthy ? 'ok' : 'warn'"></span>
            <span>{{ healthy ? '采集引擎正常' : '采集引擎有异常' }}</span>
          </div>
          <div class="engine-row"><span>门店在线</span><b>{{ onlineStores }} / {{ statuses.length }}</b></div>
          <div class="engine-row"><span>巨懂车账号</span><b>{{ statuses.length }}</b></div>
          <button class="engine-row engine-link" type="button" title="打开设置" @click="ui.view = 'settings'">
            <span>巨量引擎应用</span>
            <b>→</b>
          </button>
          <button v-if="!healthy" class="engine-alert" type="button" @click="ui.view = 'settings'">
            {{ statuses.find((s) => s.last_error)?.store.name }} 拉取失败 →
          </button>
        </div>
        <button class="rail-status" :aria-label="healthText" :title="healthText" @click="ui.rail = false">
          <span class="dot" :class="healthy ? 'ok' : 'warn'"></span>
        </button>
      </div>
    </aside>

    <div class="main">
      <header class="topbar">
        <div class="page-head">
          <h1>{{ pageHead.title }}</h1>
          <p>{{ pageHead.sub }}</p>
        </div>
        <div class="top-actions">
          <label class="store-filter">
            <span>门店</span>
            <select v-model.number="ui.storeId" class="control" aria-label="按门店筛选">
              <option :value="null">全部门店</option>
              <option v-for="s in stores" :key="s.id" :value="s.id">{{ s.name }}</option>
            </select>
          </label>
          <span class="sync" :class="{ 'is-busy': pulling }">
            <span class="dot" :class="pulling ? 'warn' : 'ok'"></span>
            <span>{{ syncText }}</span>
          </span>
          <button v-if="ui.view !== 'settings'" class="btn btn-primary" :disabled="pulling" @click="pullNow">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
              <path d="M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6" />
            </svg>
            <span>{{ pulling ? '拉取中…' : '立即拉取' }}</span>
          </button>
        </div>
      </header>

      <main class="content">
        <DashboardView v-if="ui.view === 'dashboard'" />
        <BidView v-else-if="ui.view === 'bid'" />
        <LeadsView v-else-if="ui.view === 'leads'" />
        <SettingsView v-else />
      </main>
    </div>
  </div>

  <div class="toast" :class="{ 'is-on': ui.toast }" role="status" aria-live="polite">{{ ui.toast }}</div>
</template>
