//! Parsing.
//!
//! Every diagnostic is reported in one pass rather than stopping at the first one, so the
//! caller sees all the places that need changing at once.

use crate::ast::{Document, Item, Node, Op, PathType, Rule, VERSION};
use crate::diag::{Diagnostic, DiagnosticKind};
use crate::lex::{lex, LineKind, Malformed, Sym};
use crate::span::Span;

/// Parses a ctree document.
///
/// A single [`crate::Severity::Error`] anywhere makes this return `Err`, because the library
/// builds no partial AST by recovering from errors, and the `Err` carries every diagnostic
/// rather than only the first one.
///
/// A successful parse can still leave warnings, but the `Ok` value holds a [`Document`]
/// alone, since warnings are re-derived from the document by [`crate::check`]. The empty
/// document warning is the only one the parse itself produces, and it is decidable from the
/// AST anyway. That leaves the caller one place to look for warnings.
///
/// # Arguments
///
/// * `src` - the source text of the whole document
pub fn parse(src: &str) -> Result<Document, Vec<Diagnostic>> {
    let (lines, mut diags) = lex(src);

    let base = find_base(&lines, &mut diags);

    let mut p = Parser {
        lines: &lines,
        idx: 0,
        base,
        diags: Vec::new(),
    };
    let items = p.scope(0);
    diags.append(&mut p.diags);

    let doc = Document {
        version: VERSION,
        root: Node {
            name: None,
            path_type: PathType::Recursive,
            span: Span::new(1, 1, 0, src.len()),
            items,
        },
    };

    // An empty document is worth reporting only when nothing else is wrong, since saying so
    // on a page full of syntax errors would only be noise.
    if doc.stats().nodes == 0 && !diags.iter().any(Diagnostic::is_error) {
        diags.push(Diagnostic::new(
            DiagnosticKind::EmptyDocument,
            doc.root.span,
        ));
    }

    if diags.iter().any(Diagnostic::is_error) {
        return Err(diags);
    }
    Ok(doc)
}

/// Returns the base indentation, which is the smallest positive indentation found among the
/// declaration lines.
///
/// Comment and blank lines take no part, because their indentation follows the author's
/// taste and does not express structure.
///
/// # Arguments
///
/// * `lines` - every line of the document, already classified
/// * `diags` - collects a diagnostic when the base is none of 2, 4 or 8
fn find_base(lines: &[crate::lex::Line], diags: &mut Vec<Diagnostic>) -> usize {
    let mut base = 0usize;
    for line in lines {
        if matches!(line.kind, LineKind::Decl { .. }) && line.indent > 0 {
            base = if base == 0 {
                line.indent
            } else {
                base.min(line.indent)
            };
        }
    }
    if base == 0 {
        // No line is indented at all, so any non zero value will do, since every indented
        // line will then be judged a jump.
        return 2;
    }
    if !matches!(base, 2 | 4 | 8) {
        diags.push(Diagnostic::new(
            DiagnosticKind::IndentBase { got: base },
            Span::new(1, 1, 0, 0),
        ));
    }
    base
}

struct Parser<'a> {
    lines: &'a [crate::lex::Line],
    idx: usize,
    base: usize,
    diags: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn skip_trivia(&mut self) {
        while self.idx < self.lines.len() {
            match self.lines[self.idx].kind {
                LineKind::Blank | LineKind::Comment => self.idx += 1,
                _ => break,
            }
        }
    }

    /// Parses the entries of one scope, which sit at the given indentation.
    ///
    /// # Arguments
    ///
    /// * `indent` - the indentation the entries of this scope sit at
    fn scope(&mut self, indent: usize) -> Vec<Item> {
        let lines = self.lines;
        let mut items: Vec<Item> = Vec::new();
        loop {
            self.skip_trivia();
            if self.idx >= lines.len() {
                break;
            }
            let line = &lines[self.idx];
            if line.indent < indent {
                break;
            }
            // Test for a multiple of the base before testing for a jump, because the jump
            // branch continues the loop, and with the order reversed a mixed indentation
            // would never be reported. An indentation that is not a multiple of the base is
            // always greater than the indentation expected here.
            if self.base != 0 && line.indent % self.base != 0 {
                self.diags.push(Diagnostic::new(
                    DiagnosticKind::IndentMixed {
                        indent: line.indent,
                    },
                    line.span,
                ));
            }
            if line.indent > indent {
                self.diags
                    .push(Diagnostic::new(DiagnosticKind::IndentJump, line.span));
                self.idx += 1;
                continue;
            }

            let span = line.span;
            match &line.kind {
                LineKind::Blank | LineKind::Comment => unreachable!("already skipped"),
                LineKind::Prose => {
                    self.diags
                        .push(Diagnostic::new(DiagnosticKind::UnexpectedLine, span));
                    self.idx += 1;
                }
                LineKind::Malformed { reason, .. } => {
                    let kind = match reason {
                        Malformed::MissingSeparator => DiagnosticKind::MissingSeparator,
                        Malformed::EmptyName => DiagnosticKind::EmptyName,
                    };
                    self.diags.push(Diagnostic::new(kind, span));
                    self.idx += 1;
                }
                LineKind::Decl { sym, name } => {
                    let sym = *sym;
                    let name: Box<str> = name.clone();
                    self.idx += 1;
                    let inner = self.scope(indent + self.base);
                    self.finish(sym, name, span, inner, &mut items);
                }
            }
        }
        items
    }

    fn finish(
        &mut self,
        sym: Sym,
        name: Box<str>,
        span: Span,
        inner: Vec<Item>,
        items: &mut Vec<Item>,
    ) {
        match sym {
            Sym::At => items.push(Item::Rule(Rule {
                op: Op::Subtree,
                color: name,
                span,
                targets: Vec::new(),
            })),
            Sym::Gt => {
                let mut targets = Vec::new();
                for item in inner {
                    match item {
                        Item::Child(n) => targets.push(n),
                        // A rule has to land in the scope of some node, and the block of a
                        // `>` rule admits node declarations only.
                        Item::Rule(r) => self
                            .diags
                            .push(Diagnostic::new(DiagnosticKind::RuleOutsideScope, r.span)),
                    }
                }
                if targets.is_empty() {
                    self.diags
                        .push(Diagnostic::new(DiagnosticKind::EmptyTargets, span));
                }
                items.push(Item::Rule(Rule {
                    op: Op::Explicit,
                    color: name,
                    span,
                    targets,
                }));
            }
            Sym::Dash | Sym::Pipe | Sym::Tilde => {
                let path_type = match sym {
                    Sym::Dash => PathType::Recursive,
                    Sym::Pipe => PathType::Stop,
                    _ => PathType::Isolated,
                };
                if path_type == PathType::Stop && !inner.is_empty() {
                    self.diags
                        .push(Diagnostic::new(DiagnosticKind::StopHasChildren, span));
                }
                items.push(Item::Child(Node {
                    name: Some(name),
                    path_type,
                    span,
                    items: inner,
                }));
            }
        }
    }
}
