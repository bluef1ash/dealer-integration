import type { DashboardTotals, Lead, PlanRow, StoreSummary } from './types';

/** 假数据：接真实采集（#5/#6/#7）后整体删除 */

export const MOCK_STORES: StoreSummary[] = [
  {
    id: 'A', name: '星辰汽车 · 城东店', account: '13800000001', collectOk: true,
    lastCollect: '今天 10:32', leads: 34, spend: 4820, deals: 4,
    trend: [12, 15, 11, 18, 22, 19, 25, 21, 28, 24, 30, 27, 33, 34],
  },
  {
    id: 'B', name: '星辰汽车 · 高新店', account: '13800000002', collectOk: true,
    lastCollect: '今天 10:31', leads: 29, spend: 4150, deals: 3,
    trend: [10, 12, 14, 11, 16, 15, 19, 17, 21, 20, 24, 22, 26, 29],
  },
  {
    id: 'C', name: '星辰汽车 · 滨江店', account: '13800000003', collectOk: false,
    lastCollect: '2 小时前', leads: 23, spend: 3510, deals: 2,
    trend: [14, 13, 15, 12, 16, 14, 13, 17, 15, 14, 18, 16, 15, 23],
  },
];

export const MOCK_TOTALS: DashboardTotals = { leads: 86, spend: 12480, deals: 9 };

export const MOCK_PLANS: PlanRow[] = [
  { name: '[低价引流] 秦PLUS 首页信息流', spend: 2180, leads: 16, state: '投放中' },
  { name: '[到店] 周末试驾邀约-智能推荐', spend: 1450, leads: 11, state: '投放中' },
  { name: '[品牌] 星辰全系人群拓量', spend: 1190, leads: 7, state: '已暂停' },
];

export const MOCK_LEADS: Lead[] = [
  { id: 'L01', time: '今天 10:18', store: '城东店', carModel: '秦PLUS DM-i', phone: '13900001001', status: '未跟进', sourcePlan: '[低价引流] 秦PLUS' },
  { id: 'L02', time: '今天 09:52', store: '高新店', carModel: '宋PLUS EV', phone: '13900001002', status: '已跟进', sourcePlan: '[到店] 周末试驾' },
  { id: 'L03', time: '今天 09:31', store: '城东店', carModel: '汉 EV', phone: '13900001003', status: '已到店', sourcePlan: '[品牌] 全系拓量' },
  { id: 'L04', time: '今天 09:05', store: '滨江店', carModel: '海豚', phone: '13900001004', status: '未跟进', sourcePlan: '[低价引流] 海豚' },
  { id: 'L05', time: '今天 08:47', store: '城东店', carModel: '秦PLUS DM-i', phone: '13900001005', status: '已成交', sourcePlan: '[低价引流] 秦PLUS' },
  { id: 'L06', time: '昨天 21:33', store: '高新店', carModel: '唐 DM-p', phone: '13900001006', status: '已跟进', sourcePlan: '[到店] 周末试驾' },
  { id: 'L07', time: '昨天 20:11', store: '滨江店', carModel: '宋 Pro DM-i', phone: '13900001007', status: '未跟进', sourcePlan: '[品牌] 全系拓量' },
  { id: 'L08', time: '昨天 18:26', store: '城东店', carModel: '海鸥', phone: '13900001008', status: '无效线索', sourcePlan: '—' },
  { id: 'L09', time: '昨天 17:40', store: '高新店', carModel: '秦PLUS DM-i', phone: '13900001009', status: '已到店', sourcePlan: '[低价引流] 秦PLUS' },
  { id: 'L10', time: '昨天 16:02', store: '城东店', carModel: '汉 DM-i', phone: '13900001010', status: '已跟进', sourcePlan: '[到店] 周末试驾' },
];
