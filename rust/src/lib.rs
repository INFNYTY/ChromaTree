//! ChromaTree, abbreviated ctree, is a declarative language for tinting trees.
//!
//! A document describes a tree of nodes together with the rules that tint them. Rules take
//! effect in declaration order, so a later rule overrides an earlier one. Each node carries a
//! path type, which decides whether it accepts the tint reaching it from its ancestors and
//! whether it hands its own tint down to its children.
//!
//! The grammar is five symbols. A `-` path accepts a tint from an ancestor and propagates it,
//! a `|` path accepts one and stops there, and a `~` path refuses the one from above and starts
//! a tint of its own. An `@` rule tints the whole subtree of the node it sits in, and a `>`
//! rule tints only the direct children it lists.
//!
//! The library reads no file and no environment variable, and takes the real tree as a sequence
//! of segments from the caller. It knows no business vocabulary, so a word list wherever one is
//! needed comes from the caller. It produces no natural language either, since a [`Diagnostic`]
//! carries only a kind and a position and the wording belongs to the caller. Names are opaque
//! to it and it neither splits nor joins them, having no notion of a separator.
//!
//! # Examples
//!
//! ```
//! let doc = chromatree::parse("
//! - root
//!     @ default
//!     > special
//!         | fileA
//! ").expect("the document should parse");
//!
//! // A path is a sequence of segments, and the caller decides how to split one.
//! assert_eq!(doc.color_of(&["root", "fileA"]), Some("special"));
//! assert_eq!(doc.color_of(&["root", "fileB"]), Some("default"));
//! // The world is open, so a descendant that is not declared still gets the default tint.
//! assert_eq!(doc.color_of(&["root", "deep", "down"]), Some("default"));
//! ```
//!
//! The specification and the tutorial are in `docs/`.

mod ast;
mod check;
mod diag;
mod eval;
mod fmt;
mod lex;
mod parse;
mod span;

pub use ast::{Document, Item, Node, Op, PathType, Rule, Stats, Version, VERSION};
pub use check::{check, check_vocabulary};
pub use diag::{all_diagnostic_codes, Diagnostic, DiagnosticKind, Severity};
pub use eval::{evaluate, Coloring, Decision, Declaration, Via};
pub use parse::parse;
pub use span::Span;
