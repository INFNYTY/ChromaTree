# 贡献与维护

**简体中文** | [English](CONTRIBUTING.en-US.md)

## 仓库的形状

```
docs/            共享的规范级文档（中英各一份）
conformance/     一致性语料库（数据）
examples/        人读的示例
rust/            一个实现。将来的 go/ js/ 与它平级，各自带清单与测试
```

根目录没有任何语言的构建清单。每个实现自带一个目录，互不干扰。

## 开发环境

Rust 实现：稳定版工具链，无平台特定依赖。

```
cd rust
cargo test
cargo clippy --all-targets -- -D warnings -D clippy::disallowed_methods
cargo fmt --all -- --check
```

第二条不是可选的：`clippy.toml` 的 `disallowed-methods` 把 `std::fs::*`、`std::env::*`
等一整串列为禁止，`-D clippy::disallowed_methods` 让它们变成编译错误。这是“实现不碰
文件系统与环境”这条边界唯一的强制形式，没有它，那句话就只是文档里的一句好话。

唯一豁免是 `rust/tests/conformance.rs`：读语料必须做 IO。那条
`#![allow(clippy::disallowed_methods)]` 写在文件顶部并附了理由，豁免本身就该是显式的，
让人一眼看见。

## 改代码必须同步改文档

这是硬性要求。最容易漏的几项：

| 改动 | 需要同步 |
|---|---|
| 语法、词法、符号语义 | `docs/*/spec.md`，并升语言版本 |
| 规范里的例子、编写规范 | `docs/*/tutorial.md` |
| 语言的语义（颜色怎么传播、谁是块根……） | `conformance/` 的语料 + `docs/*/contract.md` |
| 契约里的操作面或边界 | `docs/*/contract.md` + 各语言目录里的 `docs/*/api.md` |
| 诊断 kind 或短码 | `conformance/codes.json` + `docs/*/contract.md` 的清单 |
| 某语言的公开 API | 该语言的 `docs/*/api.md`、`README` |
| 新增或变更 Cargo feature | `rust/docs/*/api.md`、`rust/README*` |
| 新增设计取舍（含被否决的方案） | `docs/*/design.md` |
| 版本号 | 各语言自己的清单；`conformance/` 每个文件的 `spec` 字段 |
| 新增示例 | `examples/` 放文件，`conformance/` 加一条用 `source_file` 的用例 |

语料是规范的可执行形式。改了规范不改语料，等于没改，下一个人只会看到一份自相矛盾的
仓库。反过来也成立：语料跑不过就不算实现了这一版，没有例外。

## 双语

文档与 README 均为中英双语。修改其中一份时必须同步另一份：

- 根目录级：`X.md`（中文）↔ `X.en-US.md`（英文）
- `docs/` 下：`docs/zh-CN/X.md` ↔ `docs/en-US/X.md`
- 各语言目录下同样：`rust/docs/zh-CN/api.md` ↔ `rust/docs/en-US/api.md`
- `docs/README.md` 是唯一的语言索引，一份文件里放中英两张表

## 链接写法

仓库内的文档一律用相对链接，这样 GitHub、Gitee 与本地编辑器都能解析。

`rust/README.md` 是例外，它用绝对链接指向主站。它是 crate 的首页，crates.io 会从发布
包里单独渲染它，而仓库里的其他文件不在那个包里；crates.io 那段改写逻辑只认 GitHub，
还按“crate 名就是仓库根下的目录名”去猜路径，本仓库的 crate 在 `rust/` 下、包名是
`chromatree`，正好是它猜错的那一类。相对链接在那里要么失效，要么指到不存在的地址。

绝对链接里的分支名是 `main`，改分支名时这些链接要一起改。

## 加一个语言实现

1. 建顶层目录 `go/`、`js/`……，自带该语言的清单与测试。
2. 写一个 harness 读 `conformance/` 并跑全部用例，这是“实现完成”的定义。
3. 加 `docs/zh-CN/api.md` 与 `docs/en-US/api.md`，写该语言的签名；开头声明实现的是
   规范的哪一版。
4. 在根 `README` 的实现表里加一行，在 `docs/README.md` 里加一行。
5. 不要动根目录：根目录不放任何语言的构建清单。

## 发布与标签

- 标签统一成 `<语言>/v<版本>`，例如 `rust/v0.3.0`。Go 的子目录模块必须是这个形状；
  其余生态不强制，但统一才不用记两套。
- 各语言注册表各自查名。项目名始终是 ChromaTree、语言简称始终是 ctree；注册表里叫
  什么（crates.io 上的 `chromatree`、npm 上的、Go 模块路径）是各生态自己的事，
  不要求同名。
- 从各语言目录里发布：`cd rust && cargo publish`。

### 两条跟 git 依赖有关的约束

Cargo 会在 git 仓库里任意位置寻找包的 `Cargo.toml`（不必在根目录），所以 `rust/`
独立成包、根目录不放清单是可行的。但有个不明显的边界：

一、`rust/` 下只能有一个工作区。Cargo 会扫描整个仓库去找你要的那个包，但对那个包
自己的依赖不做同样的爬取，若它 `path` 依赖到同一仓库里另一个工作区的 crate，
会以 `no matching package named 'B' found` 收场。现在只有一个包、没有 path 依赖，
安全；将来若要加第二个 Rust crate，必须放进同一个工作区，不能另起一个。

二、`no matching package` 这个报错会骗人。它同样可能来自完全无关的解析失败
（例如某个依赖指向了访问不到的 registry），而被吞掉成这一条。调 git 依赖失败时，
别一上来就怀疑布局。

## 注释风格

- 注释一律英文。中文只出现在文档里：`docs/`、`rust/docs/*/api.md`，以及 README 与
  CONTRIBUTING 的中文版。
- 只写 `///` 声明性文档注释，说明为什么与约束是什么，不复述代码。
- 绝大多数条目一句话说清功能，用普通英文标点。
- 带参数的方法写 `# Arguments` 小节，参数一行一条，小写，句尾不加句号。
- 方法体内不写解释“这行在做什么”的注释。
- 凡是有反直觉的地方（例如为什么某处必须共用一份计算、为什么某条豁免是必要的），
  在那里写清楚。
- 构建与工具配置（`Cargo.toml`、`clippy.toml`）不写注释，理由写在 README 或本文件里。
  `.gitignore` 是例外，忽略规则的理由只有它自己能写。
- 文档正文平铺直叙、连续行文，不用加粗强调，不用破折号，也不用“三件事需要注意”这类
  只报幕不说话的开头。

## 测试

- 规范与教程里的每个例子都要进 `conformance/`，它们是规范的一部分，回归时第一个
  该被检查。
- 各语言自己的测试只放搬不走的：类型层面的断言、序列化、属性测试。加用例之前先问
  一句：它是规范要求的，还是本实现特有的？
- 实现是纯的，所以尽量用属性测试：`~` 的屏蔽性、不相交作用域的顺序无关性、
  求值确定性、写回的幂等。
- 别让语料库空转。它必须真的在跑：改坏一处求值逻辑，它就该挂。一个永远通过的测试
  等于没有测试。

## 提交信息

一行标题，说清“改了什么”；需要时在正文里说“为什么”。中英不限，
与仓库既有提交保持一致。
