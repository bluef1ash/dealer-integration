<script setup lang="ts">
import type { FetchState } from '../types';

/**
 * 异步数据区的统一呈现：加载/空/错误三种状态各占一块，
 * ready 时渲染插槽内容。空态与错误态都给出下一步动作提示。
 */
defineProps<{ state: FetchState }>();
</script>

<template>
  <div v-if="state === 'loading'" class="state-block">
    <span class="spinner" aria-hidden="true"></span>
    <p>数据加载中…</p>
  </div>
  <div v-else-if="state === 'empty'" class="state-block">
    <p class="state-title">暂无数据</p>
    <p class="state-hint">请先在「设置」中添加门店并登录，或点击「立即拉取」。</p>
  </div>
  <div v-else-if="state === 'error'" class="state-block error">
    <p class="state-title">数据获取失败</p>
    <p class="state-hint">请检查网络与门店会话状态，然后重试。</p>
    <button class="retry" type="button">重试</button>
  </div>
  <slot v-else />
</template>

<style scoped>
.state-block {
  border: 1px dashed #e7e5e4;
  border-radius: 16px;
  padding: 48px 24px;
  text-align: center;
  color: #78716c;
  background: #fff;
}
.state-block.error {
  border-color: #fecaca;
  background: #fef2f2;
  color: #991b1b;
}
.state-title {
  font-size: 16px;
  font-weight: 600;
  color: #44403c;
  margin-bottom: 6px;
}
.error .state-title {
  color: #991b1b;
}
.state-hint {
  font-size: 13px;
  color: #a8a29e;
}
.retry {
  margin-top: 14px;
  border: 1px solid #4f46e5;
  color: #4f46e5;
  background: none;
  border-radius: 10px;
  padding: 7px 20px;
  font-size: 13px;
  cursor: pointer;
}
.spinner {
  display: inline-block;
  width: 22px;
  height: 22px;
  border: 3px solid #e7e5e4;
  border-top-color: #4f46e5;
  border-radius: 50%;
  animation: spin 0.9s linear infinite;
  margin-bottom: 10px;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
