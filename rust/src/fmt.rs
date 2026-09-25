//! Canonical writing back to source.
//!
//! The output is idempotent, so writing a parsed document out again gives the same text
//! character for character.
//!
//! Comments are not preserved, because the AST holds none and they belong to the lexical
//! layer. That makes this a canonicalizer rather than a formatter, and running it over a
//! hand written document erases the comments in it. See `docs/*/api.md`.

use std::fmt::Write as _;

use crate::ast::{Document, Item, Node, Op, PathType};

/// The base indentation the canonical output uses.
const INDENT: usize = 4;

impl Document {
    /// Writes the document back to source in canonical form, which is idempotent.
    pub fn to_source(&self) -> String {
        let mut s = String::new();
        write_items(&mut s, &self.root.items, 0);
        s
    }
}

fn write_items(out: &mut String, items: &[Item], depth: usize) {
    for item in items {
        match item {
            Item::Child(n) => write_node(out, n, depth),
            Item::Rule(r) => {
                let sym = match r.op {
                    Op::Subtree => '@',
                    Op::Explicit => '>',
                };
                pad(out, depth);
                let _ = writeln!(out, "{sym} {}", r.color);
                if r.op == Op::Explicit {
                    // A target is held by the rule and is also a child of the node, so
                    // rendering it under the rule reads the same way as the source does.
                    for t in &r.targets {
                        write_node(out, t, depth + 1);
                    }
                }
            }
        }
    }
}

fn write_node(out: &mut String, node: &Node, depth: usize) {
    let Some(name) = node.name() else {
        // The anonymous root is never rendered on its own.
        write_items(out, &node.items, depth);
        return;
    };
    let sym = match node.path_type {
        PathType::Recursive => '-',
        PathType::Stop => '|',
        PathType::Isolated => '~',
    };
    pad(out, depth);
    let _ = writeln!(out, "{sym} {name}");
    write_items(out, &node.items, depth + 1);
}

fn pad(out: &mut String, depth: usize) {
    for _ in 0..depth * INDENT {
        out.push(' ');
    }
}
