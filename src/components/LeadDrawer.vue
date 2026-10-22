<script setup lang="ts">
import { NButton } from 'naive-ui'
import { computed } from 'vue'
import type { LeadDetail } from '../types'
import { fmtFull } from '../utils'

const props = defineProps<{ lead: LeadDetail | null }>()
const emit = defineEmits<{ close: [] }>()

const open = computed(() => props.lead !== null)

const rows = computed(() => {
  const l = props.lead
  if (!l) {
    return []
  }
  return [
    ['询价城市', l.city || '—'],
    ['意向车型', l.intention || '—'],
    ['线索类型', l.source_label || '—'],
    ['流量类型', l.traffic_type || '—'],
    ['转化状态', l.conversion_status || '—'],
    ['来源场景', l.scene || '—'],
    ['来源计划', l.promotion_name || '自然流量'],
    ['门店', l.store_name],
    ['线索 ID', l.leads_id],
  ]
})

async function copyPhone(): Promise<void> {
  if (!props.lead) {
    return
  }
  await navigator.clipboard.writeText(props.lead.phone)
}
</script>

<template>
  <div class="scrim" :class="{ 'is-open': open }" @click="emit('close')"></div>
  <aside class="drawer" :class="{ 'is-open': open }" aria-label="线索详情" :aria-hidden="!open">
    <div class="drawer-head">
      <div>
        <p class="eyebrow">线索详情</p>
        <h2>{{ lead?.name || '未命名' }}</h2>
      </div>
      <button class="icon-btn" aria-label="关闭线索详情" @click="emit('close')">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
          <path d="m6 6 12 12M18 6 6 18" />
        </svg>
      </button>
    </div>
    <div v-if="lead" class="drawer-body">
      <div class="drawer-status">
        <span class="pill" :class="{ 'pill-solid': lead.status_label === '已成交' }">{{ lead.status_label || '未知' }}</span>
        <span class="meta">{{ fmtFull(lead.leads_time) }}</span>
      </div>
      <div class="phone-block">
        <span class="num">{{ lead.phone }}</span>
        <n-button size="small" @click="copyPhone">复制</n-button>
      </div>
      <dl class="detail-list">
        <template v-for="[k, v] in rows" :key="k">
          <dt>{{ k }}</dt>
          <dd>{{ v }}</dd>
        </template>
      </dl>
    </div>
  </aside>
</template>
