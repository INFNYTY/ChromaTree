# chromatree —— Rust 的 API

[English](../en-US/api.md) | **简体中文**

> 本实现实现的是规范 0.3。
>
> 语言规则见 [spec.md](../../../docs/zh-CN/spec.md)；任何实现都要满足的边界与语义见
> [contract.md](../../../docs/zh-CN/contract.md)（那份是共享的，本文件只写 Rust 这边
> 的样子）。符合与否的判据是 [conformance/](../../../conformance/) 里的语料。
>
> 包名是 `chromatree`，因为 `ctree` 在 crates.io 上被一个树克隆的 crate 占了。
> 语言本身仍简称 ctree。

---

## 1. 包与 feature

```toml
[dependencies]
chromatree = "0.3"
```

| feature | 默认 | 作用 |
|---|---|---|
| `serde` | 开 | `Document` / `Node` / `Span` 等的 `Serialize` / `Deserialize` |

关掉它，crate 只依赖 `std`。

## 2. 四条边界在 Rust 里怎么强制

契约里的四条边界（零 IO、零业务词汇、零自然语言、名字不透明）中，第一条可以
编译期强制：

- `clippy.toml` 的 `disallowed-methods` 列了 `std::fs::*`、`std::env::*` 等一整串，
  CI 与本地都跑 `-D clippy::disallowed_methods`，违反了就编译不过，不是靠自觉。
- `Cargo.toml` 的 `[lints.rust] unsafe_code = "forbid"`。

第三条靠 `Display for Diagnostic` 只输出 `code@line:col`、并有一条断言它全 ASCII 的
测试来锁死。

## 3. 类型

```rust
pub struct Document {
    pub version: Version,
    pub root: Node,          // 匿名根，name 为 None
}

pub struct Node {
    pub name: Option<Box<str>>,   // None 仅出现在匿名根
    pub path_type: PathType,
    pub span: Span,
    pub items: Vec<Item>,         // 按声明顺序
}

pub enum Item {
    Child(Node),                  // 子节点声明
    Rule(Rule),
}

pub struct Rule {
    pub op: Op,
    pub color: Box<str>,
    pub span: Span,
    /// `>` 的目标；`@` 时为空。
    pub targets: Vec<Node>,
}

pub enum Op { Subtree, Explicit }              // @  >
pub enum PathType { Recursive, Stop, Isolated } // -  |  ~

pub struct Span { pub line: u32, pub col: u32, pub start: usize, pub end: usize }
```

`line` 与 `col` 都从 1 起，`col` 指向声明的符号，也就是缩进 + 1。
`start..end` 是绝对字节区间，可以直接拿去给原文切片，它对 BOM 与 CRLF 都算对，
而这两处最容易错。

（早年这里写过“`col` 按 Unicode 标量计”。那条规则不可观测：符号之前只可能有
ASCII 空格，按标量与按字节算必然相等，而库不会报出任何更靠右的位置。留着它只会
让实现者以为有这么一道坎。）

同一路径可以声明多次（spec §2.8）：AST 里它们是分开的两个节点，求值时由
`resolve_chain` 把它们归到同一层，路径类型取行号最大的那条声明，两边的规则都参与。
`Node::child` 只返回第一个，不要拿它做路径解析；它留给“只看这一层的声明”的场合。

规则持有自己的 `targets`。`>` 的目标写在它自己的块里，同时它们也是所属节点的孩子
（spec §4.2）。为避免同一个节点存两份，它们只存在规则里，于是“一个节点的孩子”
= `items` 里的 `Item::Child` 加上各条规则的目标，由 `Node::children` 一并给出。

`Version` 是一个 `major.minor` 常量，`to_source()` 不输出它，由 serde 携带。

手工构造 AST 时，`span.line` 必须赋有意义的值。规范 §2.5 的“行号大的规则覆盖
行号小的”全靠它；全部留成 `Span::default()` 的话所有规则同号，谁赢就变成了实现细节。
从 `parse` 拿到的文档不会有这个问题。

## 4. 解析

```rust
pub fn parse(src: &str) -> Result<Document, Vec<Diagnostic>>;
```

一次报出全部错误。只要有 `Severity::Error` 就返回 `Err`，`Document` 不产生。

成功的解析可能仍有警告，但 `Ok` 只带 `Document`：警告统一由 `check` 从文档上重新导出。
这样调用方只有一个地方要看警告。

```rust
impl Document {
    /// 规范化写回，幂等。
    ///
    /// 不保留注释，因为 AST 里没有注释，注释是词法层的东西。所以它是规范化而不是
    /// 格式化，拿它覆盖作者手写的文档会把注释抹掉。
    ///
    /// 它假定 AST 良构，手工构造的文档若违反规范（例如中止路径带了子节点），写出来的
    /// 源码自己都解析不过。判据是 `check` 返回空。
    pub fn to_source(&self) -> String;

    /// 文档声明过的每个节点，按声明顺序，给出段序列与位置。
    pub fn declared(&self) -> impl Iterator<Item = (Vec<&str>, Span)> + '_;

    pub fn stats(&self) -> Stats;
}

pub struct Stats {
    pub nodes: usize,
    pub rules: usize,
    /// 文档里出现过的颜色，去重且有序。
    pub colors: BTreeSet<Box<str>>,
}
```

`Stats::colors` 是给调用方拿去做词表校验用的，库自己不知道哪些颜色合法。
`stats` / `declared` / `to_source` 是本实现的便利 API，契约不要求。

## 5. 检查

```rust
/// 结构检查，不需要真实树。
pub fn check(doc: &Document) -> Vec<Diagnostic>;

/// 同上，外加“颜色必须在 vocab 里”。
pub fn check_vocabulary(doc: &Document, vocab: &[&str]) -> Vec<Diagnostic>;
```

空的 `vocab` 表示“不检查颜色”。kind 清单与触发条件见
[contract.md §5](../../../docs/zh-CN/contract.md#5-诊断)。

```rust
impl DiagnosticKind {
    /// 稳定短码，形如 `CT0107`。给日志与问题追踪用，不是给人读的句子。
    ///
    /// 权威表在 `conformance/codes.json`；`all_diagnostic_codes()` 让两者的一致性
    /// 能由测试保证。
    pub fn code(&self) -> &'static str;

    /// 严重度。每条诊断的严重度是固定的，不随上下文变化。
    pub fn severity(&self) -> Severity;
}

/// 本实现认识的每个 kind 的名字与短码，名字与 `codes.json` 的键逐字对应。
pub fn all_diagnostic_codes() -> Vec<(&'static str, &'static str)>;
```

## 6. 求值

```rust
impl Document {
    /// 任意路径的颜色；`None` 表示语言对此不表态（spec §3.2 的“部分”）。
    /// `&[]` 是匿名根，也不需要先给真实树。
    pub fn color_of(&self, path: &[&str]) -> Option<&str>;

    /// 影响这条路径的全部规则，按行号升序。最后一条即生效的那条。
    pub fn explain<'a>(&'a self, path: &[&str]) -> Option<Vec<Decision<'a>>>;
}

pub fn evaluate<'a>(doc: &'a Document, paths: &[Vec<String>]) -> Coloring<'a>;

impl<'a> Coloring<'a> {
    pub fn new(doc: &'a Document, paths: &[Vec<String>]) -> Self;
    pub fn paths(&self) -> &[Vec<String>];
    pub fn color_of(&self, path: &[&str]) -> Option<&'a str>;
    pub fn paths_with(&self, color: &str) -> Vec<Vec<String>>;
    pub fn roots_of(&self, color: &str) -> Vec<Decision<'a>>;
    pub fn exceptions_within(&self, root: &[&str]) -> Vec<Decision<'a>>;
    pub fn counts(&self) -> BTreeMap<&'a str, usize>;
    pub fn unmatched(&self) -> Vec<Decision<'a>>;
    pub fn explain(&self, path: &[&str]) -> Option<Vec<Decision<'a>>>;
}

pub struct Decision<'a> {
    /// 自有：查询的路径来自调用方，与文档不同寿。
    pub path: Vec<String>,
    pub color: &'a str,
    pub span: Span,
    pub via: Via,
}

pub enum Via { Subtree, Explicit, Inherited }   // @ 染的 | > 染的 | 继承来的
```

各查询的语义、以及“`roots_of` 的父路径要结构性地问”“`unmatched` 的判据”这些
易错点，见 [contract.md §3](../../../docs/zh-CN/contract.md#3-必须暴露的操作面)；
求值的精确定义见 [contract.md §4](../../../docs/zh-CN/contract.md#4-求值的精确定义)。

## 7. 诊断

```rust
pub struct Diagnostic {
    pub severity: Severity,
    pub kind: DiagnosticKind,
    pub span: Span,
    /// 与这条诊断相关的其它位置（`DeadRule` 指向覆盖它的那条规则）。
    pub related: Vec<Span>,
}

pub enum Severity { Error, Warning }
```

`Display for Diagnostic` 只输出 `code@line:col` 这样的机器可读形式
（有测试断言它全 ASCII），库不出自然语言。

`DiagnosticKind` 与 `Op` / `PathType` 等枚举不标 `#[non_exhaustive]`：
调用方应当穷尽匹配，漏了一个变体应该编译不过。

## 8. 序列化

`serde` feature 打开时，`Document` / `Node` / `Item` / `Rule` / `Span` / `Version` /
`PathType` / `Op` 以及诊断类型都可以序列化。

`Document` 的序列化形态是本实现的契约，不在跨语言的契约里（见
[contract.md §6](../../../docs/zh-CN/contract.md#6-什么不在契约里)）。
因为有 JSON 快照测试，字段改名会在那里断掉。

## 9. 稳定性

- `0.x` 期间：语义与签名的破坏性变更会升 minor（`0.3` → `0.4`）。
- 新增诊断 kind、新增查询方法不算破坏性变更。

## 10. 一致性测试

`tests/conformance.rs` 读仓库根目录 `conformance/` 的语料并跑全部用例，同时比对
`all_diagnostic_codes()` 与 `codes.json`。

它是仓库里唯一允许做 IO 的测试文件（读语料），`#![allow(clippy::disallowed_methods)]`
写在文件顶部并附了理由，库本身零 IO 由 clippy 那条约束保证，harness 不在此列。

发布到 crates.io 的包里没有它，也没有 `tests/examples.rs`：这两个测试断言的是仓库的
性质而不是包的性质，包只能装自己目录里的东西，装不下仓库根的 `conformance/` 与
`examples/`。排除写在 `Cargo.toml` 的 `exclude` 里，理由见 [Rust 的 README](../../README.md)。
