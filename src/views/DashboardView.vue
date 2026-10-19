<script setup lang="ts">
import { computed } from 'vue';
import DataState from '../components/DataState.vue';
import SparkLine from '../components/SparkLine.vue';
import { MOCK_PLANS, MOCK_STORES, MOCK_TOTALS } from '../mocks';
import type { FetchState } from '../types';
import { cpl, shortName, yuan } from '../utils';

const props = defineProps<{ state: FetchState }>();

const totals = computed(() => MOCK_TOTALS);
const costPerLead = computed(() => cpl(totals.value));
const closeRate = computed(() =>
  totals.value.leads > 0 ? ((totals.value.deals / totals.value.leads) * 100).toFixed(1) : '0.0',
);
</script>

<template>
  <DataState :state="props.state">
    <section class="hero">
      <div class="k">今日单条线索成本 · 全部门店</div>
      <div class="num">{{ yuan(costPerLead) }}<small> /条</small></div>
      <div class="meta">
        今日线索 {{ totals.leads }} 条 · 消耗 {{ yuan(totals.spend) }} · 已成交 {{ totals.deals }} 条
      </div>
    </section>

    <section class="mini">
      <div><div class="num">{{ totals.leads }}</div><div class="k">线索量</div></div>
      <div><div class="num">{{ yuan(totals.spend) }}</div><div class="k">广告消耗</div></div>
      <div><div class="num">{{ totals.deals }}</div><div class="k">已成交</div></div>
      <div><div class="num">{{ closeRate }}%</div><div class="k">成交率</div></div>
    </section>

    <section class="toolbar">
      <div class="seg">
        <button type="button">今日</button>
        <button type="button" class="on">近7天</button>
        <button type="button">近30天</button>
        <button type="button">自定义</button>
      </div>
      <button type="button" class="pull">⟳ 立即拉取</button>
    </section>

    <section class="stores">
      <article v-for="s in MOCK_STORES" :key="s.id" class="store">
        <h4>
          {{ shortName(s.name) }}
          <span v-if="!s.collectOk" class="warn">会话失效 {{ s.lastCollect }}</span>
        </h4>
        <div class="row"><span>线索 / 消耗</span><b>{{ s.leads }} 条 / {{ yuan(s.spend) }}</b></div>
        <div class="row"><span>线索成本</span><b>{{ yuan(cpl(s)) }}</b></div>
        <div class="row"><span>已成交</span><b>{{ s.deals }}</b></div>
        <div class="row">
          <span>采集</span>
          <b class="collect" :class="{ ok: s.collectOk }">
            {{ s.collectOk ? '正常' : '失效' }} · {{ s.lastCollect }}
          </b>
        </div>
        <SparkLine :points="s.trend" />
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
          <tr v-for="p in MOCK_PLANS" :key="p.name">
            <td>{{ p.name }}</td>
            <td>{{ yuan(p.spend) }}</td>
            <td>{{ p.leads }}</td>
            <td>{{ yuan(cpl(p)) }}</td>
            <td>{{ p.state }}</td>
          </tr>
        </tbody>
      </table>
    </section>
  </DataState>
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

.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; }
.seg { display: flex; gap: 6px; }
.seg button {
  border: 1px solid #e7e5e4; background: #fff; padding: 7px 16px;
  border-radius: 999px; font-size: 13px; color: #57534e; cursor: pointer;
}
.seg button.on { background: #4f46e5; border-color: #4f46e5; color: #fff; }
.pull { border: none; background: none; color: #4f46e5; font-size: 13px; text-decoration: underline; cursor: pointer; }

.stores { display: flex; gap: 16px; }
.store { flex: 1; background: #fff; border: 1px solid #e7e5e4; border-radius: 16px; padding: 18px 20px; }
.store h4 { font-size: 15px; margin-bottom: 10px; display: flex; justify-content: space-between; align-items: center; }
.row { display: flex; justify-content: space-between; font-size: 13px; color: #57534e; padding: 4px 0; }
.row b { color: #1c1917; font-weight: 600; }
.row .collect { font-weight: 400; font-size: 12px; color: #78716c; }
.row .collect.ok { color: #059669; }
.warn { font-size: 11px; background: #fef3c7; color: #92400e; border-radius: 6px; padding: 2px 8px; font-weight: 400; }

.tblwrap { background: #fff; border: 1px solid #e7e5e4; border-radius: 16px; padding: 8px 20px 16px; margin-top: 28px; }
</style>
