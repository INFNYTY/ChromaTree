# ChromaTree

**简体中文** | [English](README.en-US.md)

ChromaTree（简称 ctree）是一种声明式的树染色描述语言：在树形父子结构上，按声明顺序
对节点施加染色；后规则覆盖先规则；通过路径类型控制染色的继承与传播。

它默认真实树的子节点可能多于文档中声明的，因此描述的是“对哪些节点施加什么颜色”，
而不是枚举所有节点。颜色没有保留字，`protect`、`delete`、`red` 都只是标识符，
含义由使用者赋予。

```ctree
- root
    @ default
    > readwrite
        | fileA
        | fileB
        - pathA
    ~ pathB
        @ readonly
```

`root` 及其整棵子树染成 `default`；`fileA`、`fileB`、`pathA` 例外，染成 `readwrite`；
`pathB` 及其子树隔离于 `root` 并染成 `readonly`，此写法仅用于演示 `~` 用法；
`root` 下没在文档里声明的后代照样拿到 `default`。

## 五个符号

| 符号 | 类别 | 含义 |
|------|------|------|
| `-`  | 路径 | 递归：接受祖先染色，并向下传播 |
| `\|` | 路径 | 中止：接受祖先染色，到此为止，不向下传 |
| `~`  | 路径 | 隔离：不接受祖先染色，自成染色起点 |
| `@`  | 染色 | 给当前节点染色，颜色向下传播 |
| `>`  | 染色 | 给显式列出的直接子元素染色 |

语言本身只有一个匿名根、五个符号，外加独占一行的 `#` / `//` 注释。

## 这个仓库里有什么

这里放的是这门语言的全部：

```
docs/            规范、教程、实现契约、设计取舍（中英各一份）
conformance/     一致性语料库：每个实现都要跑，跑不过就不算实现了这一版
examples/*.ctree 人读的示例
rust/            Rust 实现（目前唯一一个）
```

语料库是这个仓库的核心资产。规范说这门语言是什么，语料库说“做到了算数”，
各语言的实现只要都跑它，分叉就是可检测的。

## 实现

| 语言 | 包 | 状态 |
|---|---|---|
| Rust | [`chromatree`](rust/)（crates.io 上的 `ctree` 已被占用） | 可用，实现规范 0.3 |
| Go / C / C++ / JS / TS | — | 计划中 |

每个实现都是原生实现，不是绑定：各自解析、各自求值，以规范与语料库为准。

## 用起来

```toml
[dependencies]
chromatree = "0.3"     # 或 git 依赖，见 rust/README.md
```

```rust
let doc = chromatree::parse(source)?;

// 路径是段序列，库不认识任何分隔符，切分由调用方决定
let segs: Vec<&str> = "root/fileA".split('/').collect();
assert_eq!(doc.color_of(&segs), Some("special"));
```

求值与检查都是纯函数：库不读文件、不读环境变量，你给它一棵树（段序列的集合），
它给你一棵带着色的树。

## 文档

[spec.md](docs/zh-CN/spec.md)（规范）、[tutorial.md](docs/zh-CN/tutorial.md)（教程）、
[contract.md](docs/zh-CN/contract.md)（实现契约）、[design.md](docs/zh-CN/design.md)（取舍）。
索引见 [docs/README.md](docs/README.md)，均有中英两份。

## 构建与测试

Rust 实现的工作目录是 `rust/`：

```
cd rust
cargo test
cargo clippy --all-targets -- -D warnings -D clippy::disallowed_methods
cargo fmt --all -- --check
```

第二条不是可选的：它保证“库不碰文件系统与环境”这条边界成立。
`cargo test` 里包含那个跑 `conformance/` 的 harness，它才是“符合规范”的判据。

维护规则见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可

MIT，见 [LICENSE](LICENSE)。
