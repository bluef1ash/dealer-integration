/** 数据加载状态：接真实数据前，各页以本状态驱动「空/加载/错误/正常」呈现。
 *  与 components/DataState.vue 同名会造成导入冲突，故类型名用 FetchState。 */
export type FetchState = 'loading' | 'empty' | 'error' | 'ready';

/** 门店汇总卡片（看板） */
export interface StoreSummary {
  id: string;
  /** 完整门店名，如「星辰汽车 · 城东店」 */
  name: string;
  /** 巨懂车账号标识（手机号） */
  account: string;
  /** 采集是否正常（会话是否有效） */
  collectOk: boolean;
  /** 最近一次成功采集的描述，如「今天 10:32」。TODO(#5)：接真实数据时改为时间戳，由视图格式化 */
  lastCollect: string;
  /** 今日线索量 */
  leads: number;
  /** 今日广告消耗（元） */
  spend: number;
  /** 今日已成交 */
  deals: number;
  /** 近 14 天线索量走势（sparkline） */
  trend: number[];
}

/** 线索明细行 */
export interface Lead {
  /** 线索 ID（巨懂车侧唯一标识）。行键用它，不用手机号 */
  id: string;
  time: string;
  store: string;
  carModel: string;
  phone: string;
  status: '未跟进' | '已跟进' | '已到店' | '已成交' | '无效线索';
  sourcePlan: string;
}

/** 投放计划行 */
export interface PlanRow {
  name: string;
  spend: number;
  leads: number;
  state: '投放中' | '已暂停';
}

/** 看板聚合（主数字 + 辅助指标） */
export interface DashboardTotals {
  leads: number;
  spend: number;
  deals: number;
}

/** 巨量引擎开放平台应用凭证（设置页保存至本地 SQLite settings 表） */
export interface OceanCredentials {
  appId: string | null;
  appSecret: string | null;
}
