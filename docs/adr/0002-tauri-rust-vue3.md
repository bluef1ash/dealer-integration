# 技术栈：Tauri 2（Rust 后端）+ Vue 3 + TypeScript

桌面壳在 Tauri 2 和 Electron 之间选择。选 Tauri：Rust 后端适合 HTTP 爬虫、SQLite、定时任务这些本项目的主体工作；Tauri 内置 WebView 正好兼作巨懂车的登录窗口（见 ADR-0003）；安装包约 10MB，自用分发方便。仓库从一开始就是 Rust 导向（.gitignore），维护者为单人 + AI 辅助，无团队语言约束。前端选 Vue 3（国内生态顺手，AI 生成质量稳定），TypeScript 全覆盖。

锁定的代价：日后换桌面壳等于重写外壳层；WebView 渲染差异（Windows 用 WebView2、macOS 用 WKWebView）需要在两端都做视觉验收。
