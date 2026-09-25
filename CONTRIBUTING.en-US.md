# Contributing and Maintenance

[简体中文](CONTRIBUTING.md) | **English**

## The Shape of the Repository

```
docs/            shared, specification-level documents (Chinese and English)
conformance/     the conformance corpus (data)
examples/        human-readable samples
rust/            one implementation. A future go/ or js/ sits beside it, with its own
                 manifest and tests
```

The repository root contains no language build manifest. Each implementation owns a
directory and stays out of the others' way.

## Environment

The Rust implementation: a stable toolchain, no platform-specific dependencies.

```
cd rust
cargo test
cargo clippy --all-targets -- -D warnings -D clippy::disallowed_methods
cargo fmt --all -- --check
```

The second is not optional: `clippy.toml`'s `disallowed-methods` forbids a whole list
including `std::fs::*` and `std::env::*`, and `-D clippy::disallowed_methods` turns those
into compile errors. This is the only enforceable form of the "no filesystem, no
environment" boundary, and without it that claim is just a nice sentence in a document.

The only exemption is `rust/tests/conformance.rs`, since reading the corpus requires IO.
That `#![allow(clippy::disallowed_methods)]` sits at the top of the file with its reason
attached. An exemption should be visible, not buried.

## Changing Code Means Changing Docs

This is a hard requirement. The easiest items to miss:

| Change | Must update |
|---|---|
| grammar, lexing, symbol semantics | `docs/*/spec.md`, and bump the language version |
| examples or style guidance in the spec | `docs/*/tutorial.md` |
| the language's semantics (how color propagates, what a region root is, …) | the corpus in `conformance/` + `docs/*/contract.md` |
| the contract's surface or boundaries | `docs/*/contract.md` + each language's `docs/*/api.md` |
| a diagnostic kind or short code | `conformance/codes.json` + the checklist in `docs/*/contract.md` |
| one language's public API | that language's `docs/*/api.md`, `README` |
| a new or changed Cargo feature | `rust/docs/*/api.md`, `rust/README*` |
| a new design trade-off (including rejected options) | `docs/*/design.md` |
| the version number | each language's own manifest; the `spec` field in every `conformance/` file |
| a new example | add the file under `examples/` and a case in `conformance/` using `source_file` |

The corpus is the executable form of the specification. Changing the spec without changing
the corpus is not changing it, and the next person just finds a self-contradictory
repository. The converse holds too: failing the corpus means the version is not
implemented, no exceptions.

## Bilingual

Docs and READMEs exist in Chinese and English. Changing one requires changing the other:

- repo root: `X.md` (Chinese) ↔ `X.en-US.md` (English)
- under `docs/`: `docs/zh-CN/X.md` ↔ `docs/en-US/X.md`
- under a language directory too: `rust/docs/zh-CN/api.md` ↔ `rust/docs/en-US/api.md`
- `docs/README.md` is the single language index, holding both tables in one file

## Writing Links

Documents inside the repository use relative links, so that GitHub, Gitee and a local
editor all resolve them.

`rust/README.md` is the exception, using absolute links to the canonical host. It is the
crate's landing page, and crates.io renders it from the published package on its own, with
the rest of the repository absent from that package. crates.io's rewriting is GitHub-only
and assumes the crate name is a directory under the repository root, while this crate sits
in `rust/` under the package name `chromatree`, which is exactly the case it gets wrong. A
relative link there either fails or points at a path that does not exist.

The absolute links name the branch `main`, so renaming the branch means updating them.

## Adding a Language Implementation

1. Create a top-level directory `go/`, `js/`, … with that language's manifest and tests.
2. Write a harness that reads `conformance/` and runs every case, which is the definition
   of "the implementation is done".
3. Add `docs/zh-CN/api.md` and `docs/en-US/api.md` with that language's signatures;
   state at the top which spec version it implements.
4. Add a row to the implementations table in the root `README` and one in
   `docs/README.md`.
5. Do not touch the repository root, since no language build manifest belongs there.

## Releasing and Tags

- Tags are uniformly `<language>/v<version>`, e.g. `rust/v0.3.0`. Go's subdirectory
  modules require that shape, and the others do not, but one convention beats two.
- Each registry's name is looked up separately. The project is always ChromaTree and the
  language always abbreviates to ctree, while what a registry calls the package
  (`chromatree` on crates.io, whatever npm gives, the Go module path) is each ecosystem's
  business, so they need not match.
- Publish from each language's directory: `cd rust && cargo publish`.

### Two Constraints Around Git Dependencies

Cargo searches a git repository for a package's `Cargo.toml` anywhere inside it, not
necessarily at the root, so `rust/` standing alone with no root manifest works. There is a
non-obvious edge:

One: only one workspace under `rust/`. Cargo scans the whole repository to find the package
you asked for, but does not do the same crawl for that package's own dependencies, so if it
`path`-depends on a crate in a different workspace of the same repository you get
`no matching package named 'B' found`. Today there is one package and no path dependency,
which is safe; if a second Rust crate is ever added it must go into the same workspace, not
a new one.

Two: the `no matching package` error lies. It can equally come from a wholly unrelated
resolution failure, a dependency pointing at an unreachable registry say, swallowed into
that one message. When debugging a failing git dependency, do not start by suspecting the
layout.

## Comment Style

- Comments are written in English. Chinese appears only in documentation: `docs/`,
  `rust/docs/*/api.md`, and the Chinese editions of README and CONTRIBUTING.
- Write only `///` declarative doc comments, explaining why and what the constraints are,
  and do not restate the code.
- Almost every item gets one sentence saying what it does, with ordinary English
  punctuation.
- A method that takes parameters gets a `# Arguments` section, one lowercase clause per
  parameter with no trailing period.
- No in-body comments explaining what a line does.
- Wherever something is counter-intuitive, why a computation must be shared or why an
  exemption is necessary, spell it out right there.
- Build and tool configuration (`Cargo.toml`, `clippy.toml`) carries no comments, and the
  reasoning goes in the README or in this file. `.gitignore` is the exception, since the
  reason for an ignore rule has nowhere else to live.
- Documentation prose is plain and connected. No bold for emphasis, no em dashes, and no
  opener that announces structure instead of saying something.

## Tests

- Every example in the spec and tutorial belongs in `conformance/`, since they are part of
  the specification and the first thing to check on a regression.
- Each language's own tests hold only what cannot move: type-level assertions,
  serialization, property tests. Before adding a case, ask whether this is required by the
  spec or specific to this implementation.
- Implementations are pure, so prefer property tests: `~`'s shielding property,
  order-independence of disjoint scopes, evaluation determinism, idempotent rendering.
- Do not let the corpus idle. It must actually run: break a piece of evaluation logic and
  it should fail, because a test that always passes is no test at all.

## Commit Messages

A one-line subject saying what changed; a body saying why when needed. Chinese or English
both fine, matching the surrounding history.
