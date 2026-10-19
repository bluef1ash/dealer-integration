<script setup lang="ts">
import DataState from '../components/DataState.vue';
import OceanCredentialsCard from '../components/OceanCredentialsCard.vue';
import { MOCK_STORES } from '../mocks';
import type { FetchState as DataStateKind } from '../types';

const props = defineProps<{ state: DataStateKind }>();
</script>

<template>
  <section class="toolbar">
    <h1>设置</h1>
  </section>

  <OceanCredentialsCard />

  <DataState :state="props.state">
    <div class="tblwrap">
      <table>
        <thead>
          <tr><th>门店</th><th>巨懂车账号</th><th>会话</th><th>最近成功采集</th><th>操作</th></tr>
        </thead>
        <tbody>
          <tr v-for="s in MOCK_STORES" :key="s.id">
            <td>{{ s.name }}</td>
            <td>{{ s.account }}</td>
            <td>
              <span :class="s.collectOk ? 'ok' : 'bad'">{{ s.collectOk ? '有效' : '失效' }}</span>
            </td>
            <td>{{ s.lastCollect }}</td>
            <td><button type="button" class="ghost">{{ s.collectOk ? '重新登录' : '立即重登' }}</button></td>
          </tr>
        </tbody>
      </table>
      <p class="actions">
        <button type="button" class="ghost">+ 添加门店</button>
        <button type="button" class="ghost">+ 授权巨量广告账户</button>
      </p>
      <p class="hint">巨量引擎授权：尚未配置（OAuth 授权后在此显示门店 ↔ 广告账户映射）</p>
      <p class="hint">巨懂车账号密码与会话存系统凭证库（Windows 凭据管理器 / macOS 钥匙串）；巨量引擎 App ID / App Secret 保存于本地数据库。</p>
    </div>
  </DataState>
</template>

<style scoped>
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; }
.toolbar h1 { font-size: 22px; }
.tblwrap { background: #fff; border: 1px solid #e7e5e4; border-radius: 16px; padding: 8px 20px 16px; }
.ok { color: #059669; }
.bad {
  color: #92400e; background: #fef3c7; border-radius: 6px;
  padding: 2px 8px; font-size: 12px;
}
.ghost {
  border: 1px solid #4f46e5; color: #4f46e5; background: none;
  border-radius: 10px; padding: 6px 14px; font-size: 13px; cursor: pointer;
}
.actions { margin-top: 12px; display: flex; gap: 10px; }
.hint { font-size: 12px; color: #a8a29e; margin-top: 8px; }
</style>
