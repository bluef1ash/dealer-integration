<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { OceanCredentials } from '../types';

const appId = ref('');
const appSecret = ref('');
/** 库中是否已保存过凭证（决定副标题文案） */
const configured = ref(false);
const loadError = ref('');
const saving = ref(false);
const formError = ref('');
const justSaved = ref(false);

onMounted(async () => {
  try {
    const c = await invoke<OceanCredentials>('get_ocean_credentials');
    appId.value = c.appId ?? '';
    appSecret.value = c.appSecret ?? '';
    configured.value = true;
  } catch (e) {
    loadError.value = String(e);
  }
});

async function save() {
  formError.value = '';
  justSaved.value = false;
  const id = appId.value.trim();
  const secret = appSecret.value.trim();
  if (!id || !secret) {
    formError.value = 'App ID 与 App Secret 均不能为空';
    return;
  }
  saving.value = true;
  try {
    await invoke('save_ocean_credentials', { appId: id, appSecret: secret });
    configured.value = true;
    justSaved.value = true;
  } catch (e) {
    formError.value = String(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="card">
    <h2>巨量引擎应用</h2>
    <template v-if="loadError">
      <p class="error">读取已保存的凭证失败：{{ loadError }}</p>
      <p class="hint">请检查本地数据库文件是否可写，重启应用后重试。</p>
    </template>
    <template v-else>
      <p class="hint">
        商业开放平台「应用管理」中的应用凭证，用于 OAuth 授权与 Marketing API 拉取报表。{{ configured ? '已配置，保存将成对覆盖。' : '尚未配置。' }}
      </p>
      <div class="fields">
        <label>
          <span>App ID</span>
          <input v-model="appId" type="text" autocomplete="off" spellcheck="false" />
        </label>
        <label>
          <span>App Secret</span>
          <input v-model="appSecret" type="password" autocomplete="new-password" spellcheck="false" />
        </label>
      </div>
      <p v-if="formError" class="error">{{ formError }}</p>
      <p class="actions">
        <button type="button" :disabled="saving" @click="save">{{ saving ? '保存中…' : '保存' }}</button>
        <span v-if="justSaved" class="ok">已保存</span>
      </p>
    </template>
  </div>
</template>

<style scoped>
.card {
  background: #fff;
  border: 1px solid #e7e5e4;
  border-radius: 16px;
  padding: 16px 20px;
  margin-bottom: 20px;
}
.card h2 { font-size: 16px; margin-bottom: 6px; }
.hint { font-size: 12px; color: #a8a29e; margin-bottom: 12px; }
.fields { display: flex; flex-direction: column; gap: 10px; max-width: 480px; }
.fields label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: #57534e; }
.fields input {
  border: 1px solid #d6d3d1;
  border-radius: 10px;
  padding: 8px 12px;
  font-size: 14px;
}
.fields input:focus { outline: 2px solid #4f46e5; outline-offset: -1px; }
.actions { margin-top: 12px; display: flex; align-items: center; gap: 12px; }
.actions button {
  border: 1px solid #4f46e5; color: #fff; background: #4f46e5;
  border-radius: 10px; padding: 7px 20px; font-size: 13px; cursor: pointer;
}
.actions button:disabled { opacity: 0.6; cursor: default; }
.ok { font-size: 13px; color: #059669; }
.error { font-size: 13px; color: #991b1b; margin-top: 8px; }
</style>
