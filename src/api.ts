/** Tauri command 的类型化封装：视图只经此层与后端通信 */
import { invoke } from '@tauri-apps/api/core'
import type {
  AdAccount,
  Dashboard,
  LeadPage,
  LeadsFilter,
  OceanCredentials,
  PullOutcome,
  Range,
  SchedulerConfig,
  SnapshotOutcome,
  Store,
  StoreStatus,
} from './types'

export const api = {
  listStores: () => invoke<Store[]>('list_stores'),
  addStore: (name: string, account: string, password: string | null) =>
    invoke<Store>('add_store', { name, account, password }),
  removeStore: (storeId: number) => invoke<void>('remove_store', { storeId }),
  getCollectionStatus: () => invoke<StoreStatus[]>('get_collection_status'),
  startLogin: (storeId: number) => invoke<void>('start_login', { storeId }),
  pullNow: (storeId: number | null) => invoke<PullOutcome[]>('pull_now', { storeId }),
  getLeads: (filter: LeadsFilter) => invoke<LeadPage>('get_leads', { filter }),
  getStatusOptions: (storeId: number | null) => invoke<string[]>('get_status_options', { storeId }),
  getDashboard: (range: Range, storeId: number | null) =>
    invoke<Dashboard>('get_dashboard', { range, storeId }),
  exportLeads: (filter: LeadsFilter) => invoke<string>('export_leads', { filter }),
  getOceanCredentials: () => invoke<OceanCredentials>('get_ocean_credentials'),
  saveOceanCredentials: (appId: string, appSecret: string) =>
    invoke<void>('save_ocean_credentials', { appId, appSecret }),
  startOceanAuth: () => invoke<{ accounts: AdAccount[] }>('start_ocean_auth'),
  listAdAccounts: () => invoke<AdAccount[]>('list_ad_accounts'),
  setStoreMapping: (storeId: number, adAccountId: string, enable: boolean) =>
    invoke<void>('set_store_mapping', { storeId, adAccountId, enable }),
  getStoreAdAccounts: (storeId: number) => invoke<string[]>('get_store_ad_accounts', { storeId }),
  runSnapshot: (storeId: number | null) => invoke<SnapshotOutcome[]>('run_snapshot', { storeId }),
  setSnapshotHour: (hour: number) => invoke<void>('set_snapshot_hour', { hour }),
  getSnapshotHour: () => invoke<number>('get_snapshot_hour'),
  getSchedulerConfig: () => invoke<SchedulerConfig>('get_scheduler_config'),
  setSchedulerConfig: (enabled: boolean, intervalMinutes: number) =>
    invoke<void>('set_scheduler_config', { enabled, intervalMinutes }),
}
