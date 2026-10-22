/** 领域类型：与 src-tauri/src/models.rs 的 serde 结构一一对应 */

/** 数据加载状态：各页以本状态驱动「空/加载/错误/正常」呈现。
 *  与 components/DataState.vue 同名会造成导入冲突，故类型名用 FetchState。 */
export type FetchState = 'loading' | 'empty' | 'error' | 'ready'

/** 门店（管理单位，每店一个巨懂车账号） */
export interface Store {
  id: number
  name: string
  /** 巨懂车账号标识（手机号） */
  account: string
}

/** 门店采集状态（设置页 + 过期角标） */
export interface StoreStatus {
  store: Store
  /** 最近成功采集 unix 秒；null 表示从未成功 */
  last_success_at: number | null
  last_error: string
  backfilled: boolean
  has_session: boolean
}

/** 单店拉取结果 */
export interface PullOutcome {
  store_id: number
  ok: boolean
  ingested: number
  session_expired: boolean
  error: string
}

/** 巨量引擎单应用报表拉取结果（竞价页手动按钮与调度共用） */
export interface OceanPullOutcome {
  app_name: string
  ok: boolean
  ingested: number
  error: string
}

/** 线索明细行（出库：列表/导出用，附带门店名） */
export interface LeadDetail {
  leads_id: string
  store_id: number
  store_name: string
  phone: string
  name: string
  status_label: string
  /** 线索类型（私聊线索/广告直接线索） */
  source_label: string
  intention: string
  /** 留资时间 unix 秒 */
  leads_time: number
  promotion_name: string
  /** 询价城市（省/市） */
  city: string
  /** 流量类型（自然经营线索/广告直接线索） */
  traffic_type: string
  /** 转化状态（私信/表单等转化动作） */
  conversion_status: string
  /** 来源场景（一级/二级） */
  scene: string
}

/** 明细页筛选下拉的全部选项 */
export interface FilterOptions {
  status: string[]
  source_label: string[]
  city: string[]
  traffic_type: string[]
  conversion_status: string[]
  scene: string[]
}

export interface LeadPage {
  total: number
  rows: LeadDetail[]
}

/** 线索筛选（page 从 0 计；page_size=0 不分页由后端导出用） */
export interface LeadsFilter {
  store_id: number | null
  status: string | null
  /** 线索类型 / 询价城市 / 流量类型 / 转化状态 / 来源场景，均精确匹配 */
  source_label: string | null
  city: string | null
  traffic_type: string | null
  conversion_status: string | null
  scene: string | null
  /** 搜索串：匹配手机号/姓名/意向车型/计划名 */
  q: string | null
  from: number | null
  to: number | null
  page: number
  page_size: number
}

/** 看板时间范围（对应 Rust Range 的 adjacently tagged serde） */
export type Range =
  | { kind: 'Today' }
  | { kind: 'Last7' }
  | { kind: 'Last30' }
  | { kind: 'Custom'; value: { from: number; to: number } }

/** 看板门店卡片 */
export interface StoreCard {
  store: Store
  leads: number
  spend: number
  /** 近 14 天线索量走势，末位为今天 */
  trend: number[]
  last_success_at: number | null
  /** 会话有效性；null 表示从未拉取 */
  session_ok: boolean | null
}

/** 投放计划扁平表行 */
export interface PlanLine {
  promotion_id: string
  promotion_name: string
  spend: number
  leads: number
  conversions: number
  impressions: number
  state: string
}

/** 看板聚合 */
export interface Dashboard {
  spend: number
  cpl: number
  leads: number
  deals: number
  /** 报表转化数合计 */
  conversions: number
  /** 报表展示数合计 */
  impressions: number
  store_cards: StoreCard[]
  plans: PlanLine[]
}

/** 巨量广告账户（OAuth 授权返回的 advertiser） */
export interface AdAccount {
  id: string
  alias: string
  /** 归属应用行 ID；null 表示来源应用已删除（报表保留，仅不再刷新） */
  ocean_app_id: number | null
  app_name: string | null
}

/** 巨量引擎应用状态（设置页列表行；Secret 不回传前端） */
export interface OceanAppStatus {
  id: number
  name: string
  /** 商业开放平台 App ID（数字串） */
  app_id: string
  /** OAuth token 过期时间 unix 秒；null 表示从未授权 */
  token_expires_at: number | null
  /** 凭证库中是否有该应用的 token */
  has_token: boolean
}

/** WebView 登录结果事件（login://result） */
export interface LoginEvent {
  store_id: number
  ok: boolean
  message: string
}

/** 自动拉取配置（设置页维护） */
export interface SchedulerConfig {
  enabled: boolean
  interval_minutes: number
}

/** 对账快照单店结果 */
export interface SnapshotOutcome {
  store_id: number
  ok: boolean
  seen: number
  error: string
}
