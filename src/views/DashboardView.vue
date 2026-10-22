<script setup lang="ts">
import { BarChart, LineChart } from 'echarts/charts'
import { GridComponent, MarkLineComponent, TooltipComponent } from 'echarts/components'
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { NButton, NDatePicker } from 'naive-ui'
import { computed, onMounted, ref, watch } from 'vue'
import VChart from 'vue-echarts'
import { api } from '../api'
import { useUiStore } from '../stores'
import type { Dashboard as Dash, FetchState, Range } from '../types'
import { fmtTime, yuan } from '../utils'

// 本页用到的 echarts 能力按需注册（vue-echarts 8 不再全局注入）
use([BarChart, LineChart, GridComponent, TooltipComponent, MarkLineComponent, CanvasRenderer])

// canvas 不认 CSS 变量，主题色按 style.less 的 :root 字面量写死，改色需两处同步
const ACCENT = 'oklch(58% 0.18 255)'
const ACCENT_BAR = 'oklch(58% 0.18 255 / 40%)'
const MUTED = 'oklch(54% 0.012 250)'
const BORDER = 'oklch(92% 0.005 250)'
const FG = 'oklch(18% 0.012 250)'

const ui = useUiStore()
const { showToast } = useUiStore()

const state = ref<FetchState>('loading')
const dash = ref<Dash | null>(null)
const rangeKey = ref<'Today' | 'Last7' | 'Last30' | 'Custom'>('Today')
const customFrom = ref('')
const customTo = ref('')

function buildRange(): Range {
  if (rangeKey.value === 'Custom') {
    const from = Math.floor(new Date(`${customFrom.value}T00:00:00`).getTime() / 1000)
    const to = Math.floor(new Date(`${customTo.value}T23:59:59`).getTime() / 1000)
    return { kind: 'Custom', value: { from, to } }
  }
  return { kind: rangeKey.value }
}

async function load(): Promise<void> {
  state.value = 'loading'
  try {
    dash.value = await api.getDashboard(buildRange(), ui.storeId)
    state.value = 'ready'
  } catch (e) {
    state.value = 'error'
    showToast(`看板加载失败：${e}`)
  }
}

function switchRange(key: 'Today' | 'Last7' | 'Last30' | 'Custom'): void {
  rangeKey.value = key
  if (key !== 'Custom') {
    void load()
  }
}

const RANGE_TEXT: Record<string, string> = { Today: '今日', Last7: '近 7 天', Last30: '近 30 天' }
const eyebrow = computed(() => {
  const scope =
    ui.storeId === null
      ? '全部门店'
      : (dash.value?.store_cards.find((c) => c.store.id === ui.storeId)?.store.name ?? '')
  return rangeKey.value === 'Custom'
    ? `${customFrom.value} 至 ${customTo.value} · ${scope}`
    : `${RANGE_TEXT[rangeKey.value]} · ${scope}`
})
const convRate = computed(() =>
  dash.value && dash.value.leads > 0
    ? ((dash.value.deals / dash.value.leads) * 100).toFixed(1)
    : '0.0',
)
const lastSuccess = computed(
  () =>
    dash.value?.store_cards
      .filter((c) => c.last_success_at)
      .map((c) => c.last_success_at)
      .sort()
      .pop() ?? null,
)

// ── 14 天走势：全站=各店趋势求和；单店=该店趋势（后端窗口固定近 14 天） ──
const series = computed(() => {
  const cards = dash.value?.store_cards ?? []
  if (ui.storeId === null) {
    const n = cards[0]?.trend.length ?? 14
    const leads = Array.from({ length: n }, (_, i) =>
      cards.reduce((a, c) => a + (c.trend[i] ?? 0), 0),
    )
    return { leads }
  }
  return { leads: cards.find((c) => c.store.id === ui.storeId)?.trend.slice() ?? [] }
})

const trendDates = computed(() => {
  const n = series.value.leads.length
  return Array.from({ length: n }, (_, i) => {
    const d = new Date(Date.now() - (n - 1 - i) * 86400_000)
    return `${d.getMonth() + 1}-${String(d.getDate()).padStart(2, '0')}`
  })
})

const trendTotal = computed(() => series.value.leads.reduce((a, b) => a + b, 0))
const trendAvg = computed(() => trendTotal.value / (series.value.leads.length || 1))

const chartDates = computed(() => {
  const d = trendDates.value
  return d.length ? `${d[0]} — ${d[d.length - 1]}` : ''
})

/// 主走势图：每日线索量柱状（末日高亮）+ 均值虚线（markLine）
const trendOption = computed(() => ({
  grid: { left: 6, right: 12, top: 28, bottom: 2, containLabel: true },
  tooltip: {
    trigger: 'axis',
    axisPointer: { type: 'shadow' },
    backgroundColor: FG,
    borderWidth: 0,
    padding: [8, 10],
    textStyle: { color: '#fff', fontSize: 11.5 },
  },
  xAxis: {
    type: 'category',
    data: trendDates.value,
    axisTick: { show: false },
    axisLine: { lineStyle: { color: BORDER } },
    axisLabel: { color: MUTED, fontSize: 11 },
  },
  yAxis: {
    type: 'value',
    splitLine: { lineStyle: { color: BORDER } },
    axisLabel: { color: MUTED, fontSize: 11 },
  },
  series: [
    {
      name: '每日线索量',
      type: 'bar',
      barMaxWidth: 26,
      itemStyle: { borderRadius: [3, 3, 0, 0] as [number, number, number, number] },
      // 最后一根（今天）用纯色高亮，其余用 40% 透明主色，与旧版视觉一致
      data: series.value.leads.map((v, i, arr) => ({
        value: v,
        itemStyle: { color: i === arr.length - 1 ? ACCENT : ACCENT_BAR },
      })),
      markLine: {
        silent: true,
        symbol: 'none',
        lineStyle: { color: MUTED, type: 'dashed', width: 1.2 },
        label: { formatter: '均值 {c}', color: MUTED, fontSize: 10.5, position: 'insideEndTop' },
        data: [{ yAxis: Math.round(trendAvg.value * 10) / 10 }],
      },
    },
  ],
}))

/// 门店卡片迷你走势：无轴平滑面积线
function sparkOption(points: number[]) {
  return {
    grid: { left: 0, right: 0, top: 2, bottom: 0 },
    xAxis: { type: 'category', show: false, data: points.map((_, i) => i) },
    yAxis: { type: 'value', show: false },
    series: [
      {
        type: 'line',
        data: points,
        smooth: true,
        symbol: 'none',
        lineStyle: { color: 'oklch(18% 0.012 250 / 42%)', width: 1.4 },
        areaStyle: { color: 'oklch(18% 0.012 250 / 6%)' },
      },
    ],
  }
}

function pickStore(id: number): void {
  ui.storeId = ui.storeId === id ? null : id
  void load()
}

watch(
  () => ui.storeId,
  () => void load(),
)
onMounted(load)
</script>

<template>
  <section v-if="state === 'error'" class="panel panel-note">
    <h2>看板加载失败</h2>
    <p class="sub">请确认门店会话与本地数据库正常，然后重试。</p>
  </section>

  <template v-else-if="dash">
    <div class="view-bar">
      <div class="seg" role="group" aria-label="时间范围">
        <button
          v-for="k in (['Today', 'Last7', 'Last30', 'Custom'] as const)"
          :key="k"
          :class="{ 'is-on': rangeKey === k }"
          @click="switchRange(k)"
        >
          {{ k === 'Today' ? '今日' : k === 'Last7' ? '近 7 天' : k === 'Last30' ? '近 30 天' : '自定义' }}
        </button>
      </div>
      <div v-if="rangeKey === 'Custom'" class="custom-range">
        <n-date-picker
          :formatted-value="customFrom"
          value-format="yyyy-MM-dd"
          size="small"
          :clearable="false"
          aria-label="开始日期"
          @update:formatted-value="customFrom = $event ?? ''"
        />
        <span class="dash">—</span>
        <n-date-picker
          :formatted-value="customTo"
          value-format="yyyy-MM-dd"
          size="small"
          :clearable="false"
          aria-label="结束日期"
          @update:formatted-value="customTo = $event ?? ''"
        />
        <n-button size="small" @click="load">应用</n-button>
      </div>
      <div class="view-meta">
        <span class="dot" :class="lastSuccess ? 'ok' : 'warn'"></span>
        <span>{{ lastSuccess ? `数据截至 ${fmtTime(lastSuccess)}` : '尚无成功采集' }}</span>
      </div>
    </div>

    <div class="kpi-band">
      <div class="kpi-main">
        <p class="eyebrow">{{ eyebrow }}</p>
        <div class="big-num">
          <span class="big-cur">¥</span><span class="big-cpl">{{ dash.cpl.toLocaleString('zh-CN') }}</span>
          <span class="big-per">/ 条</span>
        </div>
        <p class="kpi-label">单条线索成本</p>
        <p class="kpi-delta">消耗 ÷ 线索量（成交口径见右）</p>
      </div>
      <div class="kpi-side">
        <div class="kpi-cell">
          <p>线索量</p>
          <div class="kpi-cell-num"><b>{{ dash.leads.toLocaleString('zh-CN') }}</b><span>条</span></div>
        </div>
        <div class="kpi-cell">
          <p>广告消耗</p>
          <div class="kpi-cell-num"><b>{{ Math.round(dash.spend).toLocaleString('zh-CN') }}</b><span>元</span></div>
        </div>
        <div class="kpi-cell">
          <p>已成交</p>
          <div class="kpi-cell-num"><b>{{ dash.deals.toLocaleString('zh-CN') }}</b><span>条</span></div>
        </div>
        <div class="kpi-cell">
          <p>成交率</p>
          <div class="kpi-cell-num"><b>{{ convRate }}</b><span>%</span></div>
          <small>成交 ÷ 线索</small>
        </div>
      </div>
    </div>

    <section class="panel chart-panel">
      <div class="panel-head">
        <div>
          <h2>近 14 天线索走势</h2>
          <p class="sub">每日新增线索量与 14 日均值</p>
        </div>
        <div class="chart-total">
          <b>{{ trendTotal.toLocaleString('zh-CN') }}</b>
          <span>条 · 日均 <span>{{ Math.round(trendAvg).toLocaleString('zh-CN') }}</span></span>
        </div>
      </div>
      <VChart class="chart" :option="trendOption" autoresize role="img" aria-label="近 14 天每日新增线索量柱状图" />
      <div class="chart-legend">
        <span><i class="swatch solid"></i>每日线索量</span>
        <span><i class="swatch dashed"></i>14 日均值</span>
        <span class="chart-dates">{{ chartDates }}</span>
      </div>
    </section>

    <section class="block">
      <div class="block-head">
        <h2>门店表现</h2>
        <p class="sub">点击卡片筛选全站数据；会话失效的门店会标注数据截止时间</p>
      </div>
      <div class="store-grid">
        <article
          v-for="c in dash.store_cards"
          :key="c.store.id"
          class="store-card"
          :class="{ 'is-active': ui.storeId === c.store.id }"
          role="button"
          tabindex="0"
          :aria-pressed="ui.storeId === c.store.id"
          @click="pickStore(c.store.id)"
          @keydown.enter.prevent="pickStore(c.store.id)"
          @keydown.space.prevent="pickStore(c.store.id)"
        >
          <div class="store-top">
            <h3>{{ c.store.name }}</h3>
            <span v-if="c.session_ok === false" class="pill pill-warn"><span class="dot warn"></span>会话失效</span>
            <span v-else-if="c.session_ok === true" class="pill pill-ok"><span class="dot ok"></span>正常</span>
            <span v-else class="pill pill-muted">未拉取</span>
          </div>
          <div class="store-cpl">
            <template v-if="c.leads > 0">
              <span class="num">¥{{ (c.spend / c.leads).toFixed(1) }}</span><span class="unit">/ 条</span>
            </template>
            <span v-else class="num is-empty">—</span>
          </div>
          <p class="store-meta">{{ c.leads > 0 ? `线索 ${c.leads} · 消耗 ${yuan(c.spend)}` : '尚未拉取到线索' }}</p>
          <VChart class="spark" :option="sparkOption(c.trend)" autoresize aria-hidden="true" />
          <p class="store-foot">
            {{
              c.session_ok === false
                ? `会话失效 · 数据截至 ${fmtTime(c.last_success_at)}`
                : c.last_success_at
                  ? `采集于 ${fmtTime(c.last_success_at)}`
                  : '未登录巨懂车账号'
            }}
          </p>
        </article>
      </div>
    </section>
  </template>
</template>
