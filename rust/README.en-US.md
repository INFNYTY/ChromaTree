# chromatree — the Rust implementation of ChromaTree

[简体中文](https://github.com/INFNYTY/ChromaTree/blob/main/rust/README.md) | **English**

This is one implementation of [ChromaTree](https://github.com/INFNYTY/ChromaTree).
The language specification, the conformance corpus and the design rationale live at the
repository root, not in this package:

- Specification and tutorial:
  <https://github.com/INFNYTY/ChromaTree/blob/main/docs/en-US/spec.md>
- Implementation contract (the boundaries and semantics every implementation must meet):
  <https://github.com/INFNYTY/ChromaTree/blob/main/docs/en-US/contract.md>
- This implementation's API: [docs/en-US/api.md](https://github.com/INFNYTY/ChromaTree/blob/main/rust/docs/en-US/api.md)

> The package is named `chromatree`. `ctree` is taken on crates.io by a tree-cloning crate.
> The language itself is still abbreviated ctree.

## Using It

```toml
[dependencies]
chromatree = "0.3"
```

```rust
let doc = chromatree::parse("- root\n    @ default\n")?;
let segs: Vec<&str> = "root/fileA".split('/').collect();
assert_eq!(doc.color_of(&segs), Some("default"));
```

Checking and evaluation are pure: the library reads no files and no environment
variables. You hand it a tree (a set of segment sequences) and it hands back a tinted
one.

## Building and Testing

```
cargo test
cargo clippy --all-targets -- -D warnings -D clippy::disallowed_methods
cargo fmt --all -- --check
```

The second is not optional: `clippy.toml`'s `disallowed-methods` forbids
`std::fs::*` and `std::env::*`, and `-D clippy::disallowed_methods` turns those into
compile errors. This is the only enforceable form of "the library touches no filesystem
and no environment".

`tests/conformance.rs` runs the shared corpus in `conformance/` at the repository root,
which is the evidence that *this implementation conforms to spec 0.3*. It is the only test
file allowed to do IO (reading the corpus), and the exemption is written into the file.

It and `tests/examples.rs` both read the corpus and the examples from the repository root, so
they assert properties of the repository rather than of the package. A package can only carry
what is inside its own directory, and Cargo forbids `..` paths, so `Cargo.toml`'s `exclude`
leaves both files out. Without that, a crate downloaded from crates.io would not even compile
its tests, because `include_str!` would not find the files. The corpus is a repository asset,
not something to publish.

Maintenance rules: <https://github.com/INFNYTY/ChromaTree/blob/main/CONTRIBUTING.en-US.md>.

## License

MIT, see `LICENSE` at the repository root.
