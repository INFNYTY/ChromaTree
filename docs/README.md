# ChromaTree 技术文档 / Technical Documentation

**简体中文** | [English](#english)

## 共享的部分（与实现无关）

| 文档 | 内容 |
|---|---|
| [spec.md](zh-CN/spec.md) | 语言规范：核心概念、传播法则、数学原理、语法 |
| [tutorial.md](zh-CN/tutorial.md) | 教程与编写规范：怎么用这个语言 |
| [contract.md](zh-CN/contract.md) | 实现的契约：任何实现都必须满足的边界、必须暴露的操作面、诊断清单 |
| [design.md](zh-CN/design.md) | 设计取舍，含被否决的方案 |

## 各语言的实现

| 实现 | API 契约 | 说明 |
|---|---|---|
| Rust | [rust/docs/zh-CN/api.md](../rust/docs/zh-CN/api.md) | 包名 `chromatree`（`ctree` 已被占用） |

契约说的是“要能回答什么”，各语言的 API 文档说的是“用什么类型回答”，两者的分工见
[contract.md](zh-CN/contract.md) 开头。

---

## English

### Shared (implementation-independent)

| Document | Contents |
|---|---|
| [spec.md](en-US/spec.md) | The language specification: core concepts, the propagation law, mathematical basis, syntax |
| [tutorial.md](en-US/tutorial.md) | Tutorial and style guide: how to use the language |
| [contract.md](en-US/contract.md) | The implementation contract: the boundaries every implementation must meet, the required surface, the diagnostic checklist |
| [design.md](en-US/design.md) | Design rationale, including rejected alternatives |

### Implementations

| Implementation | API contract | Notes |
|---|---|---|
| Rust | [rust/docs/en-US/api.md](../rust/docs/en-US/api.md) | package `chromatree` (`ctree` is taken) |

The contract says what must be answerable, and each language's API doc says with what types.
See the top of [contract.md](en-US/contract.md).

---

文档中英双语：根目录下为 `X.md` ↔ `X.en-US.md`，`docs/` 下为 `zh-CN/` 与 `en-US/`。
改一份必须同时改另一份，对照表见 [CONTRIBUTING.md](../CONTRIBUTING.md)。
