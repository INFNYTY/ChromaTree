# chromatree — the Rust API

**English** | [简体中文](../zh-CN/api.md)

> This implementation implements spec 0.3.
>
> For the language see [spec.md](../../../docs/en-US/spec.md); for the boundaries and
> semantics *every* implementation must meet see
> [contract.md](../../../docs/en-US/contract.md) (that one is shared; this file covers
> only how Rust does it). Conformance is judged by the corpus in
> [conformance/](../../../conformance/).
>
> The package is named `chromatree`, because `ctree` is taken on crates.io by a
> tree-cloning crate. The language itself is still abbreviated ctree.

---

## 1. Package and Features

```toml
[dependencies]
chromatree = "0.3"
```

| feature | default | Effect |
|---|---|---|
| `serde` | on | `Serialize` / `Deserialize` for `Document`, `Node`, `Span` and friends |

With the feature turned off, the crate depends on `std` alone.

## 2. How Rust Enforces the Four Boundaries

Of the contract's four boundaries (no IO, no business vocabulary, no natural language,
opaque names), the first is enforceable at compile time:

- `clippy.toml`'s `disallowed-methods` lists `std::fs::*`, `std::env::*` and more, and both
  CI and local runs pass `-D clippy::disallowed_methods`, so violating it fails the build
  rather than relying on discipline.
- `Cargo.toml`'s `[lints.rust] unsafe_code = "forbid"`.

The third is pinned by `Display for Diagnostic` emitting only `code@line:col`, with a
test asserting it is entirely ASCII.

## 3. Types

```rust
pub struct Document {
    pub version: Version,
    pub root: Node,          // the anonymous root; its name is None
}

pub struct Node {
    pub name: Option<Box<str>>,   // None only for the anonymous root
    pub path_type: PathType,
    pub span: Span,
    pub items: Vec<Item>,         // in declaration order
}

pub enum Item {
    Child(Node),                  // a child declaration
    Rule(Rule),
}

pub struct Rule {
    pub op: Op,
    pub color: Box<str>,
    pub span: Span,
    /// Targets of `>`; empty for `@`.
    pub targets: Vec<Node>,
}

pub enum Op { Subtree, Explicit }              // @  >
pub enum PathType { Recursive, Stop, Isolated } // -  |  ~

pub struct Span { pub line: u32, pub col: u32, pub start: usize, pub end: usize }
```

`line` and `col` are 1-based, and `col` points at the declaration's symbol, which is the
indentation plus 1. `start..end` is an absolute byte range you can slice the original
source with, so it comes out right across a BOM and CRLF, the two places it is easiest to
get wrong.

(This used to say "`col` counts Unicode scalars". That rule is unobservable, since only
ASCII spaces can precede the symbol, so scalar counting and byte counting necessarily
agree, and the library never reports a position further right. Keeping it around would
only make implementers think there is a hurdle here.)

A path may be declared more than once (spec §2.8): in the AST they are separate nodes, and
evaluation gathers them into one level via `resolve_chain`, where the path type comes from
the highest-line-numbered declaration and the rules from both take part. `Node::child`
returns only the first, so do not use it to resolve a path; it is there for "look at this
level's declarations only".

A rule owns its `targets`. A `>`'s targets are written in its own block and are
simultaneously children of the enclosing node (spec §4.2). To avoid storing the same node
twice they live only in the rule, so "a node's children" means the `Item::Child` entries
in `items` plus every rule's targets, as yielded by `Node::children`.

`Version` is a `major.minor` constant; `to_source()` does not emit it, and serde carries it.

When constructing an AST by hand, `span.line` must carry a meaningful value. Spec §2.5's
"the higher-line-numbered rule overrides the lower" depends entirely on it, so leaving
them all at `Span::default()` gives every rule the same line number and makes the winner
an implementation detail. Documents obtained from `parse` never have this problem.

## 4. Parsing

```rust
pub fn parse(src: &str) -> Result<Document, Vec<Diagnostic>>;
```

Reports all errors at once. Any `Severity::Error` makes it return `Err`, and no `Document`
is produced.

A successful parse may still have warnings, but `Ok` carries only the `Document`, since
warnings are re-derived from the document by `check`. That way the caller has exactly one
place to look for warnings.

```rust
impl Document {
    /// Canonical rendering, which is idempotent.
    ///
    /// Comments are not preserved, since the AST has none and they belong to the lexical
    /// layer. This makes it a canonicalizer rather than a formatter, so running it over an
    /// author's hand-written document deletes their comments.
    ///
    /// It assumes a well-formed AST, and a hand-built document that violates the spec, a
    /// stop path with children say, renders source that will not even parse. The test is
    /// `check` returning empty.
    pub fn to_source(&self) -> String;

    /// Every declared node, in declaration order, as a segment sequence plus location.
    pub fn declared(&self) -> impl Iterator<Item = (Vec<&str>, Span)> + '_;

    pub fn stats(&self) -> Stats;
}

pub struct Stats {
    pub nodes: usize,
    pub rules: usize,
    /// Distinct colors appearing in the document, deduplicated and ordered.
    pub colors: BTreeSet<Box<str>>,
}
```

`Stats::colors` exists so the caller can build a vocabulary check, since the library
itself does not know which colors are legal. `stats`, `declared` and `to_source` are
conveniences of this implementation, and the contract does not require them.

## 5. Checking

```rust
/// Structural checks; needs no real tree.
pub fn check(doc: &Document) -> Vec<Diagnostic>;

/// The same, plus "every color must be in `vocab`".
pub fn check_vocabulary(doc: &Document, vocab: &[&str]) -> Vec<Diagnostic>;
```

An empty `vocab` means "do not check colors". For the list of kinds and their triggers
see [contract.md §5](../../../docs/en-US/contract.md#5-diagnostics).

```rust
impl DiagnosticKind {
    /// A stable short code such as `CT0107`. For logs and issue tracking, not a
    /// sentence for humans.
    ///
    /// The authoritative table is `conformance/codes.json`; `all_diagnostic_codes()`
    /// lets a test guarantee the two agree.
    pub fn code(&self) -> &'static str;

    /// Severity. A diagnostic's severity is fixed and does not vary by context.
    pub fn severity(&self) -> Severity;
}

/// Every kind this implementation knows, with its short code; the names match the keys
/// of `codes.json` verbatim.
pub fn all_diagnostic_codes() -> Vec<(&'static str, &'static str)>;
```

## 6. Evaluation

```rust
impl Document {
    /// The color of any path; `None` means the language declines to say (the "partial"
    /// in spec §3.2). `&[]` is the anonymous root, and no real tree is required first.
    pub fn color_of(&self, path: &[&str]) -> Option<&str>;

    /// Every rule affecting this path, ascending by line number. The last is the one in
    /// effect.
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
    /// Owned: the queried path comes from the caller and does not share the document's
    /// lifetime.
    pub path: Vec<String>,
    pub color: &'a str,
    pub span: Span,
    pub via: Via,
}

pub enum Via { Subtree, Explicit, Inherited }   // tinted by @ | tinted by > | inherited
```

For each query's semantics, and the easy-to-get-wrong points such as "`roots_of` must
query the parent structurally" and "`unmatched`'s criterion", see
[contract.md §3](../../../docs/en-US/contract.md#3-the-required-surface); for the precise
definition of evaluation see
[contract.md §4](../../../docs/en-US/contract.md#4-the-precise-definition-of-evaluation).

## 7. Diagnostics

```rust
pub struct Diagnostic {
    pub severity: Severity,
    pub kind: DiagnosticKind,
    pub span: Span,
    /// Other locations this diagnostic relates to (a `DeadRule` points at the rule that
    /// overrides it).
    pub related: Vec<Span>,
}

pub enum Severity { Error, Warning }
```

`Display for Diagnostic` emits only a machine-readable form like `code@line:col` (a test
asserts it is entirely ASCII), so the library produces no natural language.

`DiagnosticKind`, `Op`, `PathType` and friends are not `#[non_exhaustive]`: callers should
match exhaustively, and missing a variant should fail to compile.

## 8. Serialization

With the `serde` feature on, `Document`, `Node`, `Item`, `Rule`, `Span`, `Version`,
`PathType`, `Op` and the diagnostic types all serialize.

`Document`'s serialized shape is this implementation's contract, not part of the
cross-language one (see
[contract.md §6](../../../docs/en-US/contract.md#6-what-is-not-in-the-contract)). JSON
snapshot tests mean a field rename breaks there.

## 9. Stability

- During `0.x`: breaking changes to semantics or signatures bump the minor version (`0.3`
  → `0.4`).
- Adding a diagnostic kind or a query method is not a breaking change.

## 10. Conformance Testing

`tests/conformance.rs` reads the corpus in `conformance/` at the repository root, runs
every case, and compares `all_diagnostic_codes()` against `codes.json`.

It is the only test file in the repository allowed to do IO (reading the corpus), and
`#![allow(clippy::disallowed_methods)]` sits at the top of the file with its reason
attached, since the library's own no-IO guarantee comes from the clippy constraint and the
harness is not in that category.

The crate published to crates.io does not contain it, nor `tests/examples.rs`, since those
two assert properties of the repository rather than of the package: a package can only
carry what is inside its own directory, so it cannot hold the root's `conformance/` and
`examples/`. The exclusion lives in `Cargo.toml`'s `exclude`, and the reasoning is in
[the Rust README](../../README.en-US.md).
