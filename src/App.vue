<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { dateZhCN, NConfigProvider, NSelect, NTooltip, zhCN } from 'naive-ui'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { api } from './api'
import { useUiStore, type ViewKey } from './stores/index.ts'
import type { Store, StoreStatus } from './types'
import { fmtTime } from './utils'
import { themeOverrides } from './utils/theme'
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

const ui = useUiStore()
const { showToast } = useUiStore()

const stores = ref<Store[]>([])
const statuses = ref<StoreStatus[]>([])
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
// n-select 的 option 不接受 null，用 0 作「全部」哨兵，读写处与 storeId 的 null 语义互转
const storeOptions = computed(() => [
  { label: '全部门店', value: 0 },
  ...stores.value.map((s) => ({ label: s.name, value: s.id })),
])
const pageHead = computed(() => {
  const scope =
    ui.storeId === null ? '全部门店' : (stores.value.find((s) => s.id === ui.storeId)?.name ?? '')
  return { ...PAGE_HEAD[ui.view], sub: `${scope} · ${PAGE_HEAD[ui.view].sub}` }
})
const syncText = computed(() =>
  lastPullAt.value ? `${fmtTime(lastPullAt.value)} 已同步` : '尚未同步',
)

async function refreshStatus(): Promise<void> {
  try {
    statuses.value = await api.getCollectionStatus()
  } catch (e) {
    showToast(`读取采集状态失败：${e}`)
  }
}

async function toggleMaximize(): Promise<void> {
  await win.toggleMaximize()
}

function markSynced(): void {
  lastPullAt.value = Math.floor(Date.now() / 1000)
}

// 拉取按钮在各页自持（线索页拉巨懂车、竞价页拉巨量），侧栏只负责同步时间与采集状态
async function onLeadsPulled(): Promise<void> {
  markSynced()
  await refreshStatus()
}

function onOceanPulled(): void {
  markSynced()
}

onMounted(async () => {
  stores.value = await api.listStores().catch(() => [])
  await refreshStatus()
  window.addEventListener('leads-pulled', onLeadsPulled)
  window.addEventListener('ocean-pulled', onOceanPulled)
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
  window.removeEventListener('leads-pulled', onLeadsPulled)
  window.removeEventListener('ocean-pulled', onOceanPulled)
  for (const off of unlisteners) {
    off()
  }
})
</script>

<template>
  <n-config-provider :theme-overrides="themeOverrides" :locale="zhCN" :date-locale="dateZhCN">
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
        <!-- 与应用图标同构的极简标记：灰矮柱 + 白高柱 + 蓝色圆头（复刻 icons/app-icon.png） -->
        <svg viewBox="0 0 24 24" fill="none">
          <path d="M7 19v-3.5a2.1 2.1 0 0 1 4.2 0V19Z" fill="#8A93A5" />
          <path d="M12.8 19V9a2.1 2.1 0 0 1 4.2 0v10Z" fill="#F5F7FA" />
          <circle cx="14.9" cy="7.6" r="3" fill="#2E7FE8" />
        </svg>
      </div>
      <b>经销商运营台</b>
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
          <n-tooltip
            v-for="item in g.items"
            :key="item.key"
            trigger="hover"
            placement="right"
            :show-arrow="false"
          >
            <template #trigger>
              <button
                class="nav-item"
                :class="{ 'is-active': ui.view === item.key }"
                @click="ui.view = item.key"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
                  <path :d="item.icon" />
                </svg>
                <span>{{ item.label }}</span>
              </button>
            </template>
            {{ item.label }}
          </n-tooltip>
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
        <n-tooltip trigger="hover" placement="right" :show-arrow="false">
          <template #trigger>
            <button class="rail-status" :aria-label="healthText" @click="ui.rail = false">
              <span class="dot" :class="healthy ? 'ok' : 'warn'"></span>
            </button>
          </template>
          {{ healthText }}
        </n-tooltip>
      </div>
    </aside>

    <div class="main">
      <header class="topbar">
        <div class="page-head">
          <h1>{{ pageHead.title }}</h1>
          <p>{{ pageHead.sub }}</p>
        </div>
        <div class="top-actions">
          <div class="store-filter">
            <span>门店</span>
            <n-select
              :value="ui.storeId ?? 0"
              :options="storeOptions"
              :consistent-menu-width="false"
              aria-label="按门店筛选"
              @update:value="ui.storeId = $event || null"
            />
          </div>
          <span class="sync">
            <span class="dot ok"></span>
            <span>{{ syncText }}</span>
          </span>
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
  </n-config-provider>
</template>
