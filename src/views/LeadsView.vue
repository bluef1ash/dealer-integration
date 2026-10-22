<script setup lang="ts">
import { NButton, NInput, NSelect } from 'naive-ui'
import { computed, onMounted, ref, watch } from 'vue'
import { api } from '../api'
import LeadDrawer from '../components/LeadDrawer.vue'
import { useUiStore } from '../stores/index.ts'
import type { FetchState, LeadDetail, LeadPage, LeadsFilter } from '../types'
import { fmtFull } from '../utils'

const ui = useUiStore()
const { showToast } = useUiStore()

const PAGE_SIZE = 8
const state = ref<FetchState>('loading')
const data = ref<LeadPage | null>(null)
const statuses = ref<string[]>([])
const exporting = ref(false)

const q = ref('')
// n-select 的 option 不接受 null，用空串作「全部状态」哨兵，过滤参数处转回 null
const status = ref('')
type Period = 'Today' | 'Last7' | 'All'
const period = ref<Period>('Last7')
const page = ref(0)
const totalPages = ref(0)
const current = ref<LeadDetail | null>(null)

const statusOptions = computed(() => [
  { label: '全部状态', value: '' },
  ...statuses.value.map((s) => ({ label: s, value: s })),
])

function onStatusChange(value: string): void {
  status.value = value
  page.value = 0
  void load()
}

function currentFilter(): LeadsFilter {
  const now = Math.floor(Date.now() / 1000)
  const dayStart = Math.floor(new Date(new Date().setHours(0, 0, 0, 0)).getTime() / 1000)
  const bounds: Record<Period, [number | null, number | null]> = {
    Today: [dayStart, now],
    Last7: [dayStart - 6 * 86400, now],
    All: [null, null],
  }
  const [from, to] = bounds[period.value]
  return {
    store_id: ui.storeId,
    status: status.value || null,
    q: q.value.trim() || null,
    from,
    to,
    page: page.value,
    page_size: PAGE_SIZE,
  }
}

async function load(): Promise<void> {
  state.value = 'loading'
  try {
    data.value = await api.getLeads(currentFilter())
    totalPages.value = Math.max(1, Math.ceil(data.value.total / PAGE_SIZE))
    state.value = 'ready'
  } catch (e) {
    state.value = 'error'
    showToast(`线索加载失败：${e}`)
  }
}

watch(q, () => {
  page.value = 0
  void load()
})
watch(
  () => ui.storeId,
  () => {
    page.value = 0
    void load()
  },
)

async function exportExcel(): Promise<void> {
  exporting.value = true
  try {
    const path = await api.exportLeads(currentFilter())
    showToast(`已导出：${path}`)
  } catch (e) {
    showToast(`导出失败：${e}`)
  } finally {
    exporting.value = false
  }
}

async function go(delta: number): Promise<void> {
  const next = page.value + delta
  if (next < 0 || next >= totalPages.value) {
    return
  }
  page.value = next
  await load()
}

onMounted(async () => {
  statuses.value = await api.getStatusOptions(null).catch(() => [])
  await load()
})
</script>

<template>
  <div class="toolbar">
    <div class="toolbar-left">
      <n-input
        v-model:value="q"
        class="search"
        size="small"
        placeholder="搜索手机号、姓名、车型或计划"
        aria-label="搜索线索"
        clearable
      >
        <template #prefix>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <circle cx="11" cy="11" r="6.5" />
            <path d="m16 16 4 4" />
          </svg>
        </template>
      </n-input>
      <n-select
        :value="status"
        :options="statusOptions"
        :consistent-menu-width="false"
        aria-label="按线索状态筛选"
        @update:value="onStatusChange"
      />
      <div class="seg" role="group" aria-label="留资时间">
        <button :class="{ 'is-on': period === 'Today' }" @click="period = 'Today'; page = 0; load()">今日</button>
        <button :class="{ 'is-on': period === 'Last7' }" @click="period = 'Last7'; page = 0; load()">近 7 天</button>
        <button :class="{ 'is-on': period === 'All' }" @click="period = 'All'; page = 0; load()">全部</button>
      </div>
    </div>
    <n-button :disabled="exporting" @click="exportExcel">
      {{ exporting ? '导出中…' : '导出 Excel' }}
    </n-button>
  </div>

  <section v-if="state === 'error'" class="panel panel-note">
    <h2>线索加载失败</h2>
    <p class="sub">请检查本地数据库与会话状态后重试。</p>
  </section>

  <section v-else class="panel table-panel">
    <div class="table-headline">
      <h2>线索明细</h2>
      <span class="lead-count">共 {{ data?.total ?? 0 }} 条</span>
    </div>
    <div class="table-scroll">
      <table class="ds-table leads-table">
        <thead>
          <tr>
            <th>留资时间</th>
            <th>门店</th>
            <th>意向车型</th>
            <th>客户</th>
            <th>手机号</th>
            <th>状态</th>
            <th>来源计划</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="!data?.rows.length">
            <td colspan="8" class="empty-row">没有符合条件的线索，调整筛选或先在设置页完成采集。</td>
          </tr>
          <tr v-for="l in data?.rows ?? []" :key="l.leads_id" @click="current = l">
            <td><span class="lead-time">{{ fmtFull(l.leads_time) }}</span></td>
            <td>{{ l.store_name }}</td>
            <td>{{ l.intention || '—' }}</td>
            <td>{{ l.name || '未命名' }}</td>
            <td><span class="phone-cell">{{ l.phone }}</span></td>
            <td>
              <span
                class="pill"
                :class="{ 'pill-solid': l.status_label === '已成交', 'pill-warn': l.status_label === '未联系' }"
              >{{ l.status_label || '未知' }}</span>
            </td>
            <td><span class="plan-cell">{{ l.promotion_name || '自然流量' }}</span></td>
            <td class="chev">›</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div class="pager">
      <span>第 {{ page + 1 }} / {{ totalPages }} 页</span>
      <n-button size="tiny" :disabled="page === 0" @click="go(-1)">上一页</n-button>
      <n-button size="tiny" :disabled="page + 1 >= totalPages" @click="go(1)">下一页</n-button>
    </div>
  </section>

  <LeadDrawer :lead="current" @close="current = null" />
</template>
