/** 展示层工具：货币、线索成本、门店短名、时间 */

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

/** unix 秒 → 「月-日 时:分」；空值显示占位符 */
export function fmtTime(ts: number | null | undefined): string {
  if (!ts) return '—';
  const d = new Date(ts * 1000);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getMonth() + 1}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** unix 秒 → 「YYYY-MM-DD HH:mm:ss」 */
export function fmtFull(ts: number): string {
  const d = new Date(ts * 1000);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}
