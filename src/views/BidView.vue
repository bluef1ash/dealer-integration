<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { api } from '../api'
import { useUiStore } from '../stores'
import type { Dashboard as Dash } from '../types'

const ui = useUiStore()
const { showToast } = useUiStore()

const dash = ref<Dash | null>(null)
type BidRange = 'Today' | 'Last7' | 'Last30'
const range = ref<BidRange>('Today')

async function load(): Promise<void> {
  try {
    dash.value = await api.getDashboard({ kind: range.value }, ui.storeId)
  } catch (e) {
    showToast(`竞价数据加载失败：${e}`)
  }
}

function switchRange(key: BidRange): void {
  range.value = key
  void load()
}

const plans = computed(() => dash.value?.plans ?? [])
const activePlans = computed(() => plans.value.filter((p) => p.state === '投放中').length)
const cpa = computed(() =>
  dash.value && dash.value.conversions > 0 ? dash.value.spend / dash.value.conversions : 0,
)
const totals = computed(() => ({
  spend: plans.value.reduce((a, p) => a + p.spend, 0),
  conversions: plans.value.reduce((a, p) => a + p.conversions, 0),
  impressions: plans.value.reduce((a, p) => a + p.impressions, 0),
  leads: plans.value.reduce((a, p) => a + p.leads, 0),
}))

onMounted(load)
</script>

<template>
  <div class="view-bar">
    <div class="seg" role="group" aria-label="竞价投放时间范围">
      <button :class="{ 'is-on': range === 'Today' }" @click="switchRange('Today')">今日</button>
      <button :class="{ 'is-on': range === 'Last7' }" @click="switchRange('Last7')">近 7 天</button>
      <button :class="{ 'is-on': range === 'Last30' }" @click="switchRange('Last30')">近 30 天</button>
    </div>
    <div class="view-meta">
      <span class="dot" :class="dash ? 'ok' : 'warn'"></span>
      <span>巨量引擎 Marketing API · 数据为最近一次同步口径</span>
    </div>
  </div>

  <div class="bid-summary">
    <div class="bid-cell">
      <p>消耗</p>
      <div class="bid-cell-num"><b>{{ Math.round(dash?.spend ?? 0).toLocaleString('zh-CN') }}</b><span>元</span></div>
      <small>账户层级报表合计 {{ Math.round(totals.spend).toLocaleString('zh-CN') }} 元</small>
    </div>
    <div class="bid-cell">
      <p>转化数</p>
      <div class="bid-cell-num"><b>{{ (dash?.conversions ?? 0).toLocaleString('zh-CN') }}</b><span>个</span></div>
      <small>账户层级报表口径</small>
    </div>
    <div class="bid-cell">
      <p>平均转化成本</p>
      <div class="bid-cell-num"><b>¥{{ cpa ? cpa.toFixed(1) : '—' }}</b></div>
      <small>消耗 ÷ 转化数</small>
    </div>
    <div class="bid-cell">
      <p>在投计划</p>
      <div class="bid-cell-num"><b>{{ activePlans }}</b><span>/ {{ plans.length }}</span></div>
      <small>今日有消耗视为投放中</small>
    </div>
  </div>

  <section class="panel table-panel">
    <div class="panel-head">
      <div>
        <h2>投放计划</h2>
        <p class="sub">计划层级报表 · 线索数来自线索表的计划归因</p>
      </div>
    </div>
    <div class="table-scroll">
      <table class="ds-table bid-table">
        <thead>
          <tr>
            <th>计划</th>
            <th>状态</th>
            <th class="num-col">展示数</th>
            <th class="num-col">转化数</th>
            <th class="num-col">转化率</th>
            <th class="num-col">消耗（元）</th>
            <th class="num-col">线索数</th>
            <th class="num-col">单线索成本（元）</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="!plans.length">
            <td colspan="8" class="empty-row">
              暂无计划报表：请先在设置页完成巨量引擎 OAuth 授权并等待首轮同步。
            </td>
          </tr>
          <tr v-for="p in plans" :key="p.promotion_id">
            <td><span class="plan-name">{{ p.promotion_name || p.promotion_id }}</span></td>
            <td>
              <span class="pill" :class="p.state === '投放中' ? 'pill-ok' : 'pill-muted'">{{ p.state }}</span>
            </td>
            <td class="num-col">{{ p.impressions.toLocaleString('zh-CN') }}</td>
            <td class="num-col">{{ p.conversions.toLocaleString('zh-CN') }}</td>
            <td class="num-col">{{ p.impressions ? ((p.conversions / p.impressions) * 100).toFixed(2) : '—' }}%</td>
            <td class="num-col">{{ Math.round(p.spend).toLocaleString('zh-CN') }}</td>
            <td class="num-col">{{ p.leads }}</td>
            <td class="num-col">{{ p.leads ? (p.spend / p.leads).toFixed(1) : '—' }}</td>
          </tr>
        </tbody>
        <tfoot v-if="plans.length">
          <tr>
            <td>合计 · {{ plans.length }} 个计划</td>
            <td></td>
            <td class="num-col">{{ totals.impressions.toLocaleString('zh-CN') }}</td>
            <td class="num-col">{{ totals.conversions.toLocaleString('zh-CN') }}</td>
            <td class="num-col">{{ totals.impressions ? ((totals.conversions / totals.impressions) * 100).toFixed(2) : '—' }}%</td>
            <td class="num-col">{{ Math.round(totals.spend).toLocaleString('zh-CN') }}</td>
            <td class="num-col">{{ totals.leads }}</td>
            <td class="num-col">{{ totals.leads ? (totals.spend / totals.leads).toFixed(1) : '—' }}</td>
          </tr>
        </tfoot>
      </table>
    </div>
    <p class="bid-note">
      转化率 = 转化数 ÷ 展示数；线索数按线索表的计划 ID 归因（自然流量线索不计入）；
      账户余额与现金消耗需账户资金接口，当前版本未接入。
    </p>
  </section>
</template>
