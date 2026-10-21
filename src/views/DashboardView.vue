<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api } from '../api'
import DataState from '../components/DataState.vue'
import SparkLine from '../components/SparkLine.vue'
import type { Dashboard as Dash, FetchState, Range, Store } from '../types'
import { cpl, fmtTime, shortName, yuan } from '../utils'

const state = ref<FetchState>('loading')
const stores = ref<Store[]>([])
const storeId = ref<number | null>(null)
const errorMsg = ref('')
const dash = ref<Dash | null>(null)
const pulling = ref(false)
const pullMsg = ref('')

type RangeKey = 'Today' | 'Last7' | 'Last30' | 'Custom'
const rangeKey = ref<RangeKey>('Today')
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
  errorMsg.value = ''
  try {
    dash.value = await api.getDashboard(buildRange(), storeId.value)
    state.value = dash.value.store_cards.length === 0 ? 'empty' : 'ready'
  } catch (e) {
    state.value = 'error'
    errorMsg.value = String(e)
  }
}

function switchRange(key: RangeKey): void {
  if (key === 'Custom') {
    rangeKey.value = 'Custom'
    return // 等用户选好日期点查询
  }
  rangeKey.value = key
  void load()
}

async function pullNow(): Promise<void> {
  pulling.value = true
  pullMsg.value = ''
  try {
    const outcomes = await api.pullNow(null)
    const ok = outcomes.filter((o) => o.ok)
    pullMsg.value = ok.length
      ? `已拉取 ${ok.length} 店、入库 ${ok.reduce((n, o) => n + o.ingested, 0)} 条`
      : `拉取失败：${outcomes[0]?.error ?? '无门店'}`
    await load()
  } catch (e) {
    pullMsg.value = String(e)
  } finally {
    pulling.value = false
  }
}

onMounted(async () => {
  stores.value = await api.listStores().catch(() => [])
  await load()
})
</script>

<template>
  <DataState v-if="state !== 'ready'" :state="state">
    <template v-if="state === 'error'">
      <p class="state-hint">{{ errorMsg }}</p>
      <button class="retry" type="button" @click="load">重试</button>
    </template>
  </DataState>

  <template v-else-if="dash">
    <section class="hero">
      <div class="k">单条线索成本 · 全部门店</div>
      <div class="num">{{ yuan(dash.cpl) }}<small> /条</small></div>
      <div class="meta">
        线索 {{ dash.leads }} 条 · 消耗 {{ yuan(dash.spend) }} · 已成交 {{ dash.deals }} 条
      </div>
    </section>

    <section class="mini">
      <div><div class="num">{{ dash.leads }}</div><div class="k">线索量</div></div>
      <div><div class="num">{{ yuan(dash.spend) }}</div><div class="k">广告消耗</div></div>
      <div><div class="num">{{ dash.deals }}</div><div class="k">已成交</div></div>
      <div>
        <div class="num">{{ dash.leads > 0 ? ((dash.deals / dash.leads) * 100).toFixed(1) : '0.0' }}%</div>
        <div class="k">成交率</div>
      </div>
    </section>

    <section class="toolbar">
      <div class="seg">
        <select v-model.number="storeId" class="store-select" @change="load">
          <option :value="null">全部门店</option>
          <option v-for="s in stores" :key="s.id" :value="s.id">{{ s.name }}</option>
        </select>
        <button v-for="k in ['Today', 'Last7', 'Last30']" :key="k" type="button" :class="{ on: rangeKey === k }" @click="switchRange(k as RangeKey)">
          {{ k === 'Today' ? '今日' : k === 'Last7' ? '近7天' : '近30天' }}
        </button>
        <button type="button" :class="{ on: rangeKey === 'Custom' }" @click="switchRange('Custom')">自定义</button>
        <template v-if="rangeKey === 'Custom'">
          <input v-model="customFrom" type="date" />
          <input v-model="customTo" type="date" />
          <button type="button" class="on" @click="load">查询</button>
        </template>
      </div>
      <button type="button" class="pull" :disabled="pulling" @click="pullNow">
        {{ pulling ? '拉取中…' : '⟳ 立即拉取' }}
      </button>
    </section>
    <p v-if="pullMsg" class="pull-msg">{{ pullMsg }}</p>

    <section class="stores">
      <article v-for="c in dash.store_cards" :key="c.store.id" class="store">
        <h4>
          {{ shortName(c.store.name) }}
          <span v-if="c.session_ok === false" class="warn">会话失效</span>
          <span v-else-if="c.last_success_at" class="fresh">数据截至 {{ fmtTime(c.last_success_at) }}</span>
        </h4>
        <div class="row"><span>线索 / 消耗</span><b>{{ c.leads }} 条 / {{ yuan(c.spend) }}</b></div>
        <div class="row"><span>线索成本</span><b>{{ yuan(cpl(c)) }}</b></div>
        <div class="row">
          <span>采集</span>
          <b class="collect" :class="{ ok: c.session_ok !== false }">
            {{ c.session_ok === null ? '未拉取' : c.session_ok ? '正常' : '失效' }}
            <template v-if="c.last_success_at"> · {{ fmtTime(c.last_success_at) }}</template>
          </b>
        </div>
        <SparkLine :points="c.trend" />
      </article>
    </section>

    <section class="tblwrap">
      <table>
        <thead>
          <tr>
            <th>投放计划</th><th>消耗</th><th>线索</th><th>线索成本</th><th>状态</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="p in dash.plans" :key="p.promotion_id">
            <td>{{ p.promotion_name || p.promotion_id }}</td>
            <td>{{ yuan(p.spend) }}</td>
            <td>{{ p.leads }}</td>
            <td>{{ yuan(cpl(p)) }}</td>
            <td>{{ p.state }}</td>
          </tr>
          <tr v-if="dash.plans.length === 0">
            <td colspan="5" class="none">暂无计划报表数据（授权巨量账户并拉取后显示）</td>
          </tr>
        </tbody>
      </table>
    </section>
  </template>
</template>

<style scoped>
.hero { margin-bottom: 28px; }
.hero .k { font-size: 14px; color: #78716c; }
.hero .num { font-size: 64px; font-weight: 800; letter-spacing: -2px; color: #1c1917; line-height: 1.1; }
.hero .num small { font-size: 22px; font-weight: 600; color: #a8a29e; }
.hero .meta { font-size: 13px; color: #78716c; margin-top: 6px; }

.mini { display: flex; gap: 40px; margin-bottom: 28px; }
.mini .num { font-size: 24px; font-weight: 700; color: #1c1917; }
.mini .k { font-size: 12px; color: #a8a29e; }

.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
.seg { display: flex; gap: 6px; align-items: center; }
.seg button {
  border: 1px solid #e7e5e4; background: #fff; padding: 7px 16px;
  border-radius: 999px; font-size: 13px; color: #57534e; cursor: pointer;
}
.seg button.on { background: #4f46e5; border-color: #4f46e5; color: #fff; }
.seg input { border: 1px solid #e7e5e4; border-radius: 10px; padding: 6px 10px; font-size: 13px; }
.store-select { border: 1px solid #e7e5e4; border-radius: 999px; padding: 7px 12px; background: #fff; font-size: 13px; color: #57534e; }
.pull { border: none; background: none; color: #4f46e5; font-size: 13px; text-decoration: underline; cursor: pointer; }
.pull:disabled { color: #a8a29e; text-decoration: none; cursor: default; }
.pull-msg { font-size: 12px; color: #78716c; margin-bottom: 12px; }

.stores { display: flex; gap: 16px; flex-wrap: wrap; }
.store { flex: 1; min-width: 220px; background: #fff; border: 1px solid #e7e5e4; border-radius: 16px; padding: 18px 20px; }
.store h4 { font-size: 15px; margin-bottom: 10px; display: flex; justify-content: space-between; align-items: center; gap: 6px; }
.row { display: flex; justify-content: space-between; font-size: 13px; color: #57534e; padding: 4px 0; }
.row b { color: #1c1917; font-weight: 600; }
.row .collect { font-weight: 400; font-size: 12px; color: #78716c; }
.row .collect.ok { color: #059669; }
.warn { font-size: 11px; background: #fef3c7; color: #92400e; border-radius: 6px; padding: 2px 8px; font-weight: 400; }
.fresh { font-size: 11px; color: #a8a29e; font-weight: 400; }

.tblwrap { background: #fff; border: 1px solid #e7e5e4; border-radius: 16px; padding: 8px 20px 16px; margin-top: 28px; }
.none { text-align: center; color: #a8a29e; font-size: 13px; padding: 20px 0; }
</style>
