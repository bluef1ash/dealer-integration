<script setup lang="ts">
import { ref } from 'vue'
import DashboardView from './views/DashboardView.vue'
import LeadsView from './views/LeadsView.vue'
import SettingsView from './views/SettingsView.vue'

type Page = 'dash' | 'leads' | 'settings'
const TABS: Array<{ key: Page; label: string }> = [
  { key: 'dash', label: '看板' },
  { key: 'leads', label: '线索明细' },
  { key: 'settings', label: '设置' },
]
const page = ref<Page>('dash')
</script>

<template>
  <div class="tabs">
    <a
      v-for="t in TABS"
      :key="t.key"
      :class="{ on: page === t.key }"
      @click="page = t.key"
    >{{ t.label }}</a>
  </div>

  <main class="wrap">
    <DashboardView v-if="page === 'dash'" />
    <LeadsView v-else-if="page === 'leads'" />
    <SettingsView v-else />
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
.wrap { max-width: 1080px; margin: 0 auto; padding: 32px 48px; }
</style>
