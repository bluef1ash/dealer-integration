/** 跨视图共享的界面状态：视图切换、门店筛选、同步指示、Toast。
 *  顶栏（App.vue）是唯一写「同步状态」的地方，各视图只读取。 */
import { reactive } from 'vue'

export type ViewKey = 'dashboard' | 'bid' | 'leads' | 'settings'
export type RangeKey = 'Today' | 'Last7' | 'Last30' | 'Custom'

export const ui = reactive({
  view: 'dashboard' as ViewKey,
  /** 门店筛选（null=全部）；看板顶栏与各视图共用 */
  storeId: null as number | null,
  rail: false,
  toast: '',
})

let toastTimer: ReturnType<typeof setTimeout> | undefined

export function showToast(message: string): void {
  ui.toast = message
  if (toastTimer) {
    clearTimeout(toastTimer)
  }
  toastTimer = setTimeout(() => {
    ui.toast = ''
  }, 3200)
}

export function pickStore(id: number | null): void {
  ui.storeId = id
}
