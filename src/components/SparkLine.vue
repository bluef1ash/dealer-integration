<script setup lang="ts">
/** 14 天线索走势 sparkline（纯 SVG，无图表库依赖） */
const props = defineProps<{ points: number[]; width?: number; height?: number }>();

function path(): string {
  const { points, width = 180, height = 30 } = props;
  if (points.length < 2) return '';
  const max = Math.max(...points);
  const min = Math.min(...points);
  const span = max - min || 1;
  const step = width / (points.length - 1);
  return points
    .map((p, i) => {
      const x = (i * step).toFixed(1);
      const y = (height - 2 - ((p - min) / span) * (height - 6)).toFixed(1);
      return `${i === 0 ? 'M' : 'L'}${x},${y}`;
    })
    .join(' ');
}
</script>

<template>
  <svg :width="width ?? 180" :height="height ?? 30" class="spark" aria-hidden="true">
    <path :d="path()" fill="none" stroke="#4f46e5" stroke-width="1.8" stroke-linecap="round" />
  </svg>
</template>

<style scoped>
.spark { display: block; }
</style>
