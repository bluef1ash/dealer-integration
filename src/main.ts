import { invoke } from '@tauri-apps/api/core';
import { createApp } from 'vue';
import './style.css';
import App from './App.vue';

// IPC 冒烟：验证 Tauri 桥可用；失败仅记录，不阻塞界面
invoke<string>('ping', { name: 'dealer-integration' })
  .then((msg) => console.info(`[ipc] ${msg}`))
  .catch((err) => console.warn('[ipc] 冒烟调用失败', err));

createApp(App).mount('#app');
