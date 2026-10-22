<script setup lang="ts">
import { NButton, NDatePicker, NInput, NSelect } from 'naive-ui'
import type { Ref } from 'vue'
import { onMounted, ref, watch } from 'vue'
import { api } from '../api'
import LeadDrawer from '../components/LeadDrawer.vue'
import { useUiStore } from '../stores/index.ts'
import type {
  FetchState,
  FilterOptions,
  LeadDetail,
  LeadPage,
  LeadsFilter,
  PullOutcome,
} from '../types'
import { fmtFull } from '../utils'

const ui = useUiStore()
const { showToast } = useUiStore()

const PAGE_SIZE = 8
const state = ref<FetchState>('loading')
const data = ref<LeadPage | null>(null)
const exporting = ref(false)
const pulling = ref(false)

const q = ref('')
const status = ref<string | null>(null)
const sourceLabel = ref<string | null>(null)
const city = ref<string | null>(null)
const trafficType = ref<string | null>(null)
const conversionStatus = ref<string | null>(null)
const scene = ref<string | null>(null)
/** 留资时间范围（毫秒，n-date-picker daterange）；null = 不限 */
const range = ref<[number, number] | null>(null)
const page = ref(0)
const totalPages = ref(0)
const current = ref<LeadDetail | null>(null)

const EMPTY_OPTIONS: FilterOptions = {
  status: [],
  source_label: [],
  city: [],
  traffic_type: [],
  conversion_status: [],
  scene: [],
}
const opts = ref<FilterOptions>({ ...EMPTY_OPTIONS })

const selectOptions = (values: string[]) => values.map((v) => ({ label: v, value: v }))

async function loadOptions(): Promise<void> {
  opts.value = await api.getLeadsFilterOptions(ui.storeId).catch(() => ({ ...EMPTY_OPTIONS }))
}

function currentFilter(): LeadsFilter {
  const [from, to] = range.value ?? [null, null]
  return {
    store_id: ui.storeId,
    status: status.value,
    source_label: sourceLabel.value,
    city: city.value,
    traffic_type: trafficType.value,
    conversion_status: conversionStatus.value,
    scene: scene.value,
    q: q.value.trim() || null,
    from: from === null ? null : Math.floor(from / 1000),
    to: to === null ? null : Math.floor(to / 1000) + 86_399, // 含结束日整天
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

function reload(): void {
  page.value = 0
  void load()
}

/** 维度下拉通用绑定：写入 + 重查（模板中 ref 会被解包，故在脚本侧生成处理器） */
function bindDimension(target: Ref<string | null>) {
  return (value: string | null) => {
    target.value = value
    reload()
  }
}

const onStatus = bindDimension(status)
const onSourceLabel = bindDimension(sourceLabel)
const onCity = bindDimension(city)
const onTrafficType = bindDimension(trafficType)
const onConversionStatus = bindDimension(conversionStatus)
const onScene = bindDimension(scene)

watch(q, reload)
watch(
  () => ui.storeId,
  () => {
    reload()
    void loadOptions()
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

/// 本页拉取的是巨懂车线索（竞价页拉的是巨量报表，二者各自独立）
async function pullLeads(): Promise<void> {
  if (pulling.value) {
    return
  }
  pulling.value = true
  try {
    const outcomes: PullOutcome[] = await api.pullNow(ui.storeId)
    const ok = outcomes.filter((o) => o.ok)
    showToast(
      ok.length
        ? `已拉取 ${ok.length} 家门店、入库 ${ok.reduce((n, o) => n + o.ingested, 0)} 条线索`
        : `拉取失败：${outcomes[0]?.error ?? '无门店'}`,
    )
    window.dispatchEvent(new CustomEvent('leads-pulled'))
    await load()
  } catch (e) {
    showToast(`拉取失败：${e}`)
  } finally {
    pulling.value = false
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
  await loadOptions()
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
        :options="selectOptions(opts.status)"
        size="small"
        placeholder="状态"
        clearable
        :consistent-menu-width="false"
        aria-label="按状态筛选"
        @update:value="onStatus"
      />
      <n-select
        :value="sourceLabel"
        :options="selectOptions(opts.source_label)"
        size="small"
        placeholder="线索类型"
        clearable
        :consistent-menu-width="false"
        aria-label="按线索类型筛选"
        @update:value="onSourceLabel"
      />
      <n-select
        :value="city"
        :options="selectOptions(opts.city)"
        size="small"
        placeholder="询价城市"
        clearable
        :consistent-menu-width="false"
        aria-label="按询价城市筛选"
        @update:value="onCity"
      />
      <n-select
        :value="trafficType"
        :options="selectOptions(opts.traffic_type)"
        size="small"
        placeholder="流量类型"
        clearable
        :consistent-menu-width="false"
        aria-label="按流量类型筛选"
        @update:value="onTrafficType"
      />
      <n-select
        :value="conversionStatus"
        :options="selectOptions(opts.conversion_status)"
        size="small"
        placeholder="转化状态"
        clearable
        :consistent-menu-width="false"
        aria-label="按转化状态筛选"
        @update:value="onConversionStatus"
      />
      <n-select
        :value="scene"
        :options="selectOptions(opts.scene)"
        size="small"
        placeholder="来源场景"
        clearable
        :consistent-menu-width="false"
        aria-label="按来源场景筛选"
        @update:value="onScene"
      />
      <n-date-picker
        v-model:value="range"
        type="daterange"
        size="small"
        clearable
        aria-label="留资时间范围"
        @update:value="reload"
      />
    </div>
    <div class="toolbar-actions">
      <n-button :disabled="exporting" @click="exportExcel">
        {{ exporting ? '导出中…' : '导出 Excel' }}
      </n-button>
      <n-button type="primary" :loading="pulling" @click="pullLeads">
        {{ pulling ? '拉取中…' : '拉取线索' }}
      </n-button>
    </div>
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
            <th>客户</th>
            <th>手机号</th>
            <th>询价城市</th>
            <th>意向车型</th>
            <th>线索类型</th>
            <th>流量类型</th>
            <th>转化状态</th>
            <th>来源场景</th>
            <th>状态</th>
            <th>来源计划</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="!data?.rows.length">
            <td colspan="13" class="empty-row">没有符合条件的线索，调整筛选或先在设置页完成采集。</td>
          </tr>
          <tr v-for="l in data?.rows ?? []" :key="l.leads_id" @click="current = l">
            <td><span class="lead-time">{{ fmtFull(l.leads_time) }}</span></td>
            <td>{{ l.store_name }}</td>
            <td>{{ l.name || '未命名' }}</td>
            <td><span class="phone-cell">{{ l.phone }}</span></td>
            <td>{{ l.city || '—' }}</td>
            <td>{{ l.intention || '—' }}</td>
            <td>{{ l.source_label || '—' }}</td>
            <td>{{ l.traffic_type || '—' }}</td>
            <td>{{ l.conversion_status || '—' }}</td>
            <td>{{ l.scene || '—' }}</td>
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
