<script setup lang="ts">
import { computed, ref } from 'vue';
import DataState from '../components/DataState.vue';
import { MOCK_LEADS } from '../mocks';
import type { DataState as DataStateKind } from '../types';

const props = defineProps<{ state: DataStateKind }>();

const storeFilter = ref('全部');
const statusFilter = ref('全部状态');
const STORE_OPTIONS = ['全部', '城东店', '高新店', '滨江店'];
const STATUS_OPTIONS = ['全部状态', '未跟进', '已跟进', '已到店', '已成交', '无效线索'];

const filtered = computed(() =>
  MOCK_LEADS.filter(
    (l) =>
      (storeFilter.value === '全部' || l.store === storeFilter.value) &&
      (statusFilter.value === '全部状态' || l.status === statusFilter.value),
  ),
);
</script>

<template>
  <section class="toolbar">
    <h1>线索明细</h1>
    <div class="ctrl">
      <select v-model="storeFilter">
        <option v-for="o in STORE_OPTIONS" :key="o" :value="o">{{ o }}</option>
      </select>
      <select v-model="statusFilter">
        <option v-for="o in STATUS_OPTIONS" :key="o" :value="o">{{ o }}</option>
      </select>
      <button type="button" class="ghost">导出 Excel</button>
    </div>
  </section>

  <DataState :state="props.state">
    <div class="tblwrap">
      <table>
        <thead>
          <tr>
            <th>留资时间</th><th>门店</th><th>意向车型</th><th>手机号</th><th>状态</th><th>来源计划</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="l in filtered" :key="l.phone">
            <td>{{ l.time }}</td>
            <td>{{ l.store }}</td>
            <td>{{ l.carModel }}</td>
            <td>{{ l.phone }}</td>
            <td>{{ l.status }}</td>
            <td>{{ l.sourcePlan }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </DataState>
</template>

<style scoped>
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; }
.toolbar h1 { font-size: 22px; }
.ctrl { display: flex; gap: 10px; align-items: center; }
.ctrl select {
  padding: 8px 10px; border: 1px solid #e7e5e4; border-radius: 10px;
  background: #fff; font-size: 13px; color: #44403c;
}
.ghost {
  border: 1px solid #4f46e5; color: #4f46e5; background: none;
  border-radius: 10px; padding: 8px 18px; font-size: 13px; cursor: pointer;
}
.tblwrap { background: #fff; border: 1px solid #e7e5e4; border-radius: 16px; padding: 8px 20px 16px; }
</style>
