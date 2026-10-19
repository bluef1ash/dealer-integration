# 领域文档

工程技能在探索本仓库代码时应如何使用领域文档。

本仓库为**单一上下文（single-context）**：仓库根目录一份 `GLOSSARY.md` 和一个 `docs/adr/`。

## 探索之前，先读这些

- 仓库根目录的 **`GLOSSARY.md`**
- **`docs/adr/`**：阅读与你要改动的区域相关的 ADR

如果这些文件不存在，**静默继续**。不要提示它们缺失，也不要建议预先创建。`/domain-modeling` 技能（经由 `/grill-with-docs` 和 `/improve-codebase-architecture` 触达）会在术语或决策真正确定时惰性创建它们。

## 文件结构

```
/
├── GLOSSARY.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

## 使用词汇表中的用语

当你的产出提到某个领域概念时（issue 标题、重构提案、假设、测试名），使用 `GLOSSARY.md` 中定义的术语。不要漂移到词汇表明确避免的同义词。

如果你需要的概念还不在词汇表里，这是一个信号：要么你在发明项目并不使用的语言（请重新考虑），要么存在真实缺口（记下来交给 `/domain-modeling`）。

## 标记 ADR 冲突

如果你的产出与现有 ADR 矛盾，明确指出而不是悄悄覆盖：

> _与 ADR-0007（事件溯源的订单）矛盾，但值得重新讨论，因为……_
