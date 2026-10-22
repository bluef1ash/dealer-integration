/** 跨视图共享的界面状态（Pinia）：视图切换、门店筛选、导航折叠、Toast。
 *  顶栏（App.vue）是唯一写「同步状态」的地方，各视图只读取。 */
import { defineStore } from 'pinia'
import { ref } from 'vue'

export type ViewKey = 'dashboard' | 'bid' | 'leads' | 'settings'

export const useUiStore = defineStore('ui', () => {
  const view = ref<ViewKey>('dashboard')
  /** 门店筛选（null=全部）；看板顶栏与各视图共用 */
  const storeId = ref<number | null>(null)
  const rail = ref(false)
  const toast = ref('')

  let toastTimer: ReturnType<typeof setTimeout> | undefined

  function showToast(message: string): void {
    toast.value = message
    if (toastTimer) {
      clearTimeout(toastTimer)
    }
    toastTimer = setTimeout(() => {
      toast.value = ''
    }, 3200)
  }

  return { view, storeId, rail, toast, showToast }
})
