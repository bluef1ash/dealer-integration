/** 数据加载状态：接真实数据前，各页以本状态驱动「空/加载/错误/正常」呈现 */
export type DataState = 'loading' | 'empty' | 'error' | 'ready';

/** 门店汇总卡片（看板） */
export interface StoreSummary {
  id: string;
  /** 完整门店名，如「星辰汽车 · 城东店」 */
  name: string;
  /** 巨懂车账号标识（手机号） */
  account: string;
  /** 采集是否正常（会话是否有效） */
  collectOk: boolean;
  /** 最近一次成功采集的描述，如「今天 10:32」 */
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

export function cpl(x: { leads: number; spend: number }): number {
  return x.leads > 0 ? Math.round(x.spend / x.leads) : 0;
}

export function yuan(n: number): string {
  return `¥${n.toLocaleString('zh-CN')}`;
}
