<script setup lang="ts">
import { ref } from 'vue';
import type { FetchState } from './types';
import DashboardView from './views/DashboardView.vue';
import LeadsView from './views/LeadsView.vue';
import SettingsView from './views/SettingsView.vue';

type Page = 'dash' | 'leads' | 'settings';
const TABS: Array<{ key: Page; label: string }> = [
  { key: 'dash', label: '看板' },
  { key: 'leads', label: '线索明细' },
  { key: 'settings', label: '设置' },
];
const page = ref<Page>('dash');

/** 演示控制：接真实数据后由采集/查询结果驱动，整个区块随之删除 */
const STATES: FetchState[] = ['ready', 'loading', 'empty', 'error'];
const STATE_LABELS: Record<FetchState, string> = {
  ready: '正常', loading: '加载中', empty: '空数据', error: '出错',
};
const dataState = ref<FetchState>('ready');
function cycleState(): void {
  dataState.value = STATES[(STATES.indexOf(dataState.value) + 1) % STATES.length];
}
</script>

<template>
  <div class="tabs">
    <a
      v-for="t in TABS"
      :key="t.key"
      :class="{ on: page === t.key }"
      @click="page = t.key"
    >{{ t.label }}</a>
    <span class="spacer" />
    <span class="demo">演示控制（接真实数据后移除）</span>
    <button type="button" class="demo-btn" @click="cycleState">
      数据状态：{{ STATE_LABELS[dataState] }}
    </button>
  </div>

  <main class="wrap">
    <DashboardView v-if="page === 'dash'" :state="dataState" />
    <LeadsView v-else-if="page === 'leads'" :state="dataState" />
    <SettingsView v-else :state="dataState" />
  </main>
</template>

<style scoped>
.tabs {
  display: flex; gap: 28px; align-items: center;
  padding: 0 48px; background: #fff; border-bottom: 1px solid #e7e5e4;
}
.tabs a {
  padding: 16px 4px; font-size: 15px; color: #78716c;
  cursor: pointer; border-bottom: 3px solid transparent; user-select: none;
}
.tabs a.on { color: #1c1917; border-bottom-color: #4f46e5; font-weight: 600; }
.spacer { flex: 1; }
.demo { font-size: 11px; color: #d6d3d1; }
.demo-btn {
  font-size: 12px; color: #78716c; background: #f5f5f4;
  border: 1px dashed #d6d3d1; border-radius: 999px; padding: 5px 12px; cursor: pointer;
}
.wrap { max-width: 1080px; margin: 0 auto; padding: 32px 48px; }
</style>
