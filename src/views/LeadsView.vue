<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api } from '../api'
import DataState from '../components/DataState.vue'
import type { FetchState, LeadPage, LeadsFilter, Store } from '../types'
import { fmtFull } from '../utils'

const PAGE_SIZE = 20
const state = ref<FetchState>('loading')
const errorMsg = ref('')
const stores = ref<Store[]>([])
const statuses = ref<string[]>([])
const data = ref<LeadPage | null>(null)
const exporting = ref(false)

const storeId = ref<number | null>(null)
const status = ref<string | null>(null)
type Period = 'Today' | 'Last7' | 'Last30' | 'All'
const period = ref<Period>('Last7')
const page = ref(0)
const totalPages = ref(0)

function currentFilter(): LeadsFilter {
  const now = Math.floor(Date.now() / 1000)
  const dayStart = Math.floor(new Date(new Date().setHours(0, 0, 0, 0)).getTime() / 1000)
  const bounds: Record<Period, [number | null, number | null]> = {
    Today: [dayStart, now],
    Last7: [dayStart - 6 * 86400, now],
    Last30: [dayStart - 29 * 86400, now],
    All: [null, null],
  }
  const [from, to] = bounds[period.value]
  return {
    store_id: storeId.value,
    status: status.value,
    from,
    to,
    page: page.value,
    page_size: PAGE_SIZE,
  }
}

async function load(): Promise<void> {
  state.value = 'loading'
  errorMsg.value = ''
  try {
    data.value = await api.getLeads(currentFilter())
    totalPages.value = Math.max(1, Math.ceil(data.value.total / PAGE_SIZE))
    state.value = data.value.total === 0 ? 'empty' : 'ready'
  } catch (e) {
    state.value = 'error'
    errorMsg.value = String(e)
  }
}

function switchPeriod(p: Period): void {
  period.value = p
  page.value = 0
  void load()
}

function switchStore(): void {
  page.value = 0
  void load()
}

async function switchStatus(): Promise<void> {
  page.value = 0
  void load()
}

async function go(delta: number): Promise<void> {
  const next = page.value + delta
  if (next < 0 || next >= totalPages.value) {
    return
  }
  page.value = next
  await load()
}

async function exportExcel(): Promise<void> {
  exporting.value = true
  try {
    const path = await api.exportLeads(currentFilter())
    window.alert(`已导出：\n${path}`)
  } catch (e) {
    window.alert(`导出失败：${e}`)
  } finally {
    exporting.value = false
  }
}

onMounted(async () => {
  try {
    stores.value = await api.listStores()
    statuses.value = await api.getStatusOptions(null)
  } catch (e) {
    state.value = 'error'
    errorMsg.value = String(e)
    return
  }
  await load()
})
</script>

<template>
  <section class="toolbar">
    <h1>线索明细</h1>
    <div class="ctrl">
      <select v-model.number="storeId" @change="switchStore">
        <option :value="null">全部门店</option>
        <option v-for="s in stores" :key="s.id" :value="s.id">{{ s.name }}</option>
      </select>
      <select v-model="status" @change="switchStatus">
        <option :value="null">全部状态</option>
        <option v-for="s in statuses" :key="s" :value="s">{{ s }}</option>
      </select>
      <div class="seg">
        <button v-for="p in (['Today', 'Last7', 'Last30', 'All'] as const)" :key="p" type="button" :class="{ on: period === p }" @click="switchPeriod(p)">
          {{ p === 'Today' ? '今日' : p === 'Last7' ? '近7天' : p === 'Last30' ? '近30天' : '全部' }}
        </button>
      </div>
      <button type="button" class="ghost" :disabled="exporting" @click="exportExcel">
        {{ exporting ? '导出中…' : '导出 Excel' }}
      </button>
    </div>
  </section>

  <DataState v-if="state !== 'ready'" :state="state">
    <template v-if="state === 'error'">
      <p class="state-hint">{{ errorMsg }}</p>
      <button class="retry" type="button" @click="load">重试</button>
    </template>
  </DataState>

  <template v-else-if="data">
    <div class="tblwrap">
      <table>
        <thead>
          <tr>
            <th>留资时间</th><th>门店</th><th>意向车型</th><th>手机号</th><th>状态</th><th>来源计划</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="l in data.rows" :key="l.leads_id">
            <td>{{ fmtFull(l.leads_time) }}</td>
            <td>{{ l.store_name }}</td>
            <td>{{ l.intention }}</td>
            <td>{{ l.phone }}</td>
            <td>{{ l.status_label || '—' }}</td>
            <td>{{ l.promotion_name || '—' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <p class="pager">
      共 {{ data.total }} 条 · 第 {{ page + 1 }} / {{ totalPages }} 页
      <button type="button" :disabled="page === 0" @click="go(-1)">上一页</button>
      <button type="button" :disabled="page + 1 >= totalPages" @click="go(1)">下一页</button>
    </p>
  </template>
</template>

<style scoped>
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; }
.toolbar h1 { font-size: 22px; }
.ctrl { display: flex; gap: 10px; align-items: center; }
.ctrl select {
  padding: 8px 10px; border: 1px solid #e7e5e4; border-radius: 10px;
  background: #fff; font-size: 13px; color: #44403c;
}
.seg { display: flex; gap: 4px; }
.seg button {
  border: 1px solid #e7e5e4; background: #fff; padding: 7px 12px;
  border-radius: 999px; font-size: 12px; color: #57534e; cursor: pointer;
}
.seg button.on { background: #4f46e5; border-color: #4f46e5; color: #fff; }
.ghost {
  border: 1px solid #4f46e5; color: #4f46e5; background: none;
  border-radius: 10px; padding: 8px 18px; font-size: 13px; cursor: pointer;
}
.ghost:disabled { color: #a8a29e; border-color: #e7e5e4; cursor: default; }
.tblwrap { background: #fff; border: 1px solid #e7e5e4; border-radius: 16px; padding: 8px 20px 16px; }
.pager { display: flex; gap: 12px; align-items: center; justify-content: flex-end; font-size: 13px; color: #78716c; margin-top: 12px; }
.pager button {
  border: 1px solid #e7e5e4; background: #fff; border-radius: 8px;
  padding: 5px 12px; font-size: 12px; cursor: pointer;
}
.pager button:disabled { color: #d6d3d1; cursor: default; }
</style>
