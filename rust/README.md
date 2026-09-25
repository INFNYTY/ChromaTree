# chromatree —— ChromaTree 的 Rust 实现

**简体中文** | [English](https://github.com/INFNYTY/ChromaTree/blob/main/rust/README.en-US.md)

这是 [ChromaTree](https://github.com/INFNYTY/ChromaTree) 的 Rust 实现之一。
语言规范、一致性语料库与设计取舍在仓库根目录，不在这个包里：

- 规范与教程：<https://github.com/INFNYTY/ChromaTree/blob/main/docs/zh-CN/spec.md>
- 实现契约（任何实现都必须满足的边界与语义）：
  <https://github.com/INFNYTY/ChromaTree/blob/main/docs/zh-CN/contract.md>
- 本实现的 API：[docs/zh-CN/api.md](https://github.com/INFNYTY/ChromaTree/blob/main/rust/docs/zh-CN/api.md)

> 包名是 `chromatree`。crates.io 上的 `ctree` 已被一个树克隆的 crate 占用。
> 语言本身仍简称 ctree。

## 用

```toml
[dependencies]
chromatree = "0.3"
```

```rust
let doc = chromatree::parse("- root\n    @ default\n")?;
let segs: Vec<&str> = "root/fileA".split('/').collect();
assert_eq!(doc.color_of(&segs), Some("default"));
```

求值与检查都是纯函数：库不读文件、不读环境变量。你给它一棵树（段序列的集合），
它给你一棵带着色的树。

## 构建与测试

```
cargo test
cargo clippy --all-targets -- -D warnings -D clippy::disallowed_methods
cargo fmt --all -- --check
```

第二条不是可选的：`clippy.toml` 的 `disallowed-methods` 把 `std::fs::*`、
`std::env::*` 列为禁止，`-D clippy::disallowed_methods` 让它们变成编译错误。
这是“库不碰文件系统与环境”这条承诺唯一的强制形式。

`tests/conformance.rs` 跑仓库根目录 `conformance/` 里的共享语料库，那是“这个实现
符合规范 0.3”的判据。它是唯一允许做 IO 的测试文件（读语料），豁免写在文件里。

它和 `tests/examples.rs` 都要从仓库根目录读语料与示例，所以断言的是仓库的性质，不是包
的性质。包只能装自己目录里的东西，而 Cargo 不许 `..` 路径，因此 `Cargo.toml` 的 `exclude`
把这两个文件排除在外。不排除的话，从 crates.io 下载来的 crate 连 `cargo test` 都编译
不过，因为 `include_str!` 找不到文件。语料库是仓库资产，不是发布物。

维护规则见 <https://github.com/INFNYTY/ChromaTree/blob/main/CONTRIBUTING.md>。

## 许可

MIT，见仓库根目录的 `LICENSE`。
