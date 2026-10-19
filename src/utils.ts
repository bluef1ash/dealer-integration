/** 展示层工具：货币、线索成本、门店短名 */

export function cpl(x: { leads: number; spend: number }): number {
  return x.leads > 0 ? Math.round(x.spend / x.leads) : 0;
}

export function yuan(n: number): string {
  return `¥${n.toLocaleString('zh-CN')}`;
}

/** 「星辰汽车 · 城东店」→「城东店」；无间隔号时原样返回 */
export function shortName(fullName: string): string {
  const idx = fullName.indexOf('·');
  return idx >= 0 ? fullName.slice(idx + 1).trim() : fullName;
}
