/** Naive UI 离散 API：无需在组件树挂 Provider 即可弹出确认对话框（删除等破坏性操作的二次确认）。
 *  主色为设计系统 --accent（oklch(58% 0.18 255)）的 hex 近似，naive 内部颜色合成只认 hex/rgb。 */
import { createDiscreteApi } from 'naive-ui'

const themeOverrides = {
  common: {
    primaryColor: '#1778d3',
    primaryColorHover: '#2b86dd',
    primaryColorPressed: '#1061b4',
    primaryColorSuppl: '#1778d3',
    borderRadius: '8px',
  },
}

export const { dialog } = createDiscreteApi(['dialog'], {
  configProviderProps: { themeOverrides },
})
