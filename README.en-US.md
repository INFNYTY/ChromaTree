# ChromaTree

[简体中文](README.md) | **English**

ChromaTree (abbreviated ctree) is a declarative tree-tinting language: tint nodes of a
parent/child tree in declaration order; later rules override earlier ones; path types
control how tinting is inherited and propagated.

It assumes the real tree may have more children than the document declares, so it
describes *which nodes receive which color* rather than enumerating nodes. No color is a
reserved word, and `protect`, `delete` and `red` are just identifiers whose meaning the
user assigns.

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

`root` and its whole subtree are tinted `default`; `fileA`, `fileB` and `pathA` are the
exceptions, tinted `readwrite`; `pathB` and its subtree are isolated from `root` and tinted
`readonly`, written this way only to demonstrate `~`; descendants of `root` that are never
declared still receive `default`.

## The Five Symbols

| Symbol | Kind | Meaning |
|--------|------|---------|
| `-`  | path | recursive: accepts ancestor tint and propagates it downward |
| `\|` | path | stop: accepts ancestor tint, stops here, passing nothing down |
| `~`  | path | isolated: accepts no ancestor tint, starts its own tinting |
| `@`  | tint | tints the current node; the color propagates downward |
| `>`  | tint | tints the explicitly listed direct children |

The language is one anonymous root, five symbols, and `#` / `//` comments occupying a
line of their own.

## What Is in This Repository

Here is the whole language:

```
docs/            specification, tutorial, implementation contract, rationale
                 (each in Chinese and English)
conformance/     the conformance corpus: every implementation runs it; passing it is
                 what "implements this version" means
examples/*.ctree human-readable samples
rust/            the Rust implementation (currently the only one)
```

The corpus is this repository's core asset. The specification says what the language is,
and the corpus says what counts as having done it, so as long as every implementation runs
it, divergence is detectable.

## Implementations

| Language | Package | Status |
|---|---|---|
| Rust | [`chromatree`](rust/) (`ctree` is taken on crates.io) | available, implements spec 0.3 |
| Go / C / C++ / JS / TS | — | planned |

Every implementation is a native implementation, not a binding: each parses and evaluates
on its own, judged by the specification and the corpus.

## Using It

```toml
[dependencies]
chromatree = "0.3"     # or a git dependency; see rust/README.en-US.md
```

```rust
let doc = chromatree::parse(source)?;

// paths are segment sequences, and the library knows no separator, so the caller splits
let segs: Vec<&str> = "root/fileA".split('/').collect();
assert_eq!(doc.color_of(&segs), Some("special"));
```

Checking and evaluation are pure: the library reads no files and no environment variables.
You hand it a tree (a set of segment sequences) and it hands back a tinted one.

## Documentation

[spec.md](docs/en-US/spec.md) (specification), [tutorial.md](docs/en-US/tutorial.md)
(tutorial), [contract.md](docs/en-US/contract.md) (implementation contract),
[design.md](docs/en-US/design.md) (rationale). See [docs/README.md](docs/README.md) for
the index; all four exist in both languages.

## Building and Testing

The Rust implementation's working directory is `rust/`:

```
cd rust
cargo test
cargo clippy --all-targets -- -D warnings -D clippy::disallowed_methods
cargo fmt --all -- --check
```

The second is not optional, since it is what makes "the library touches no filesystem and
no environment" hold. `cargo test` includes the harness that runs `conformance/`, and that
one is the real evidence of conformance.

Maintenance rules: [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT, see [LICENSE](LICENSE).
