<script setup lang="ts">
import { NButton, NDatePicker } from 'naive-ui'
import { computed, onMounted, ref, watch } from 'vue'
import { api } from '../api'
import { useUiStore } from '../stores'
import type { Dashboard as Dash, FetchState, Range } from '../types'
import { fmtTime, yuan } from '../utils'

const ui = useUiStore()
const { showToast } = useUiStore()

const state = ref<FetchState>('loading')
const dash = ref<Dash | null>(null)
const rangeKey = ref<'Today' | 'Last7' | 'Last30' | 'Custom'>('Today')
const customFrom = ref('')
const customTo = ref('')
const hoverBar = ref(-1)

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

const CHART_W = 760
const CHART_H = 190
const bars = computed(() => {
  const leads = series.value.leads
  const n = leads.length
  if (!n) {
    return {
      list: [] as Array<{ x: number; y: number; w: number; h: number; v: number }>,
      avg: 0,
      avgY: 0,
      total: 0,
      n,
    }
  }
  const max = Math.max(...leads, 1) * 1.16
  const gap = 12
  const bw = (CHART_W - gap * (n - 1)) / n
  const total = leads.reduce((a, b) => a + b, 0)
  const avg = total / n
  const list = leads.map((v, i) => {
    const h = Math.max(3, (v / max) * (CHART_H - 16))
    return { x: i * (bw + gap), y: CHART_H - h, w: bw, h, v }
  })
  const avgY = CHART_H - (avg / max) * (CHART_H - 16)
  return { list, avg, avgY, total, n }
})

const tipDate = (i: number): string => {
  const d = new Date(Date.now() - (bars.value.n - 1 - i) * 86400_000)
  return `${d.getMonth() + 1}-${String(d.getDate()).padStart(2, '0')}`
}
const tipLeft = (i: number): string => {
  const ratio = (i + 0.5) / (bars.value.n || 1)
  return `${Math.min(98, Math.max(2, ratio * 100))}%`
}
function onChartMove(e: MouseEvent): void {
  const width = (e.currentTarget as HTMLElement).offsetWidth
  hoverBar.value = Math.min(bars.value.n - 1, Math.floor((e.offsetX / width) * bars.value.n))
}

const chartDates = computed(() => {
  const n = bars.value.n
  return n ? `${tipDate(0)} — ${tipDate(n - 1)}` : ''
})

function sparkPath(points: number[]): { line: string; area: string } {
  const w = 120
  const h = 26
  if (!points.length) {
    return { line: '', area: '' }
  }
  const max = Math.max(...points)
  const min = Math.min(...points)
  const span = max - min || 1
  const pts = points.map((p, i) => [
    (i * w) / (points.length - 1 || 1),
    h - 2 - ((p - min) / span) * (h - 7),
  ])
  const line = pts.map((p, i) => `${i ? 'L' : 'M'}${p[0].toFixed(1)},${p[1].toFixed(1)}`).join(' ')
  const area = `${line} L${w},${h} L0,${h} Z`
  return { line, area }
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
          <b>{{ (bars.total ?? 0).toLocaleString('zh-CN') }}</b>
          <span>条 · 日均 <span>{{ Math.round(bars.avg ?? 0).toLocaleString('zh-CN') }}</span></span>
        </div>
      </div>
      <div class="chart" @mouseleave="hoverBar = -1" @mousemove="onChartMove">
        <svg :viewBox="`0 0 ${CHART_W} ${CHART_H}`" preserveAspectRatio="none" role="img" aria-label="近 14 天每日新增线索量柱状图">
          <line
            v-for="i in 3"
            :key="i"
            x1="0"
            :y1="(CHART_H / 4) * i"
            :x2="CHART_W"
            :y2="(CHART_H / 4) * i"
            stroke="var(--border)"
            stroke-width="1"
            opacity="0.55"
          />
          <rect
            v-for="(b, i) in bars.list"
            :key="i"
            class="bar"
            :class="{ 'is-peak': i === bars.n - 1, 'is-hover': hoverBar === i }"
            :x="b.x.toFixed(1)"
            :y="b.y.toFixed(1)"
            :width="b.w.toFixed(1)"
            :height="b.h.toFixed(1)"
            rx="3"
          />
          <line class="avg-line" x1="0" :y1="(bars.avgY ?? 0).toFixed(1)" :x2="CHART_W" :y2="(bars.avgY ?? 0).toFixed(1)" />
        </svg>
        <div v-if="hoverBar >= 0" class="chart-tip" :style="{ left: tipLeft(hoverBar) }">
          <b>{{ tipDate(hoverBar) }}</b>
          <span><i>线索</i>{{ bars.list[hoverBar]?.v ?? 0 }} 条</span>
        </div>
      </div>
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
          <svg class="spark" viewBox="0 0 120 26" preserveAspectRatio="none" aria-hidden="true">
            <path class="spark-area" :d="sparkPath(c.trend).area" />
            <path class="spark-line" :d="sparkPath(c.trend).line" />
          </svg>
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
