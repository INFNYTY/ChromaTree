//! Checking.
//!
//! The checks fall into two groups. The structural ones need the document alone, and the
//! semantic ones need no real tree either, since every judgement rests on the path relations
//! inside the document.
//!
//! One discipline matters throughout. The coverage test that finds a dead rule shares its
//! propagation computation with evaluation, through [`crate::eval::reaches`]. Two separate
//! implementations would drift apart sooner or later, and the checker would then call a rule
//! dead while the evaluator went on using it.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Document, Item, Node, Op, PathType};
use crate::diag::{Diagnostic, DiagnosticKind};
use crate::eval::{reaches, resolve_chain, type_at};
use crate::span::Span;

/// Runs the structural and semantic checks, which need no real tree.
///
/// # Arguments
///
/// * `doc` - the document to check
pub fn check(doc: &Document) -> Vec<Diagnostic> {
    let mut c = Checker { diags: Vec::new() };
    c.structural(doc);
    c.semantic(doc);
    c.diags
}

/// Runs the same checks and adds one requiring every color to appear in `vocab`.
///
/// # Arguments
///
/// * `doc` - the document to check
/// * `vocab` - the colors the caller accepts, where an empty slice means colors are not
///   checked
pub fn check_vocabulary(doc: &Document, vocab: &[&str]) -> Vec<Diagnostic> {
    let mut diags = check(doc);
    if vocab.is_empty() {
        return diags;
    }
    let set: BTreeSet<&str> = vocab.iter().copied().collect();
    let mut sites = Vec::new();
    collect_sites(doc, &doc.root, &mut Vec::new(), &mut sites);
    for s in sites {
        for (color, span) in s.colors {
            if !set.contains(color) {
                diags.push(Diagnostic::new(
                    DiagnosticKind::UnknownColor {
                        color: color.into(),
                    },
                    span,
                ));
            }
        }
    }
    diags
}

struct Checker {
    diags: Vec<Diagnostic>,
}

impl Checker {
    fn push(&mut self, kind: DiagnosticKind, span: Span) -> &mut Diagnostic {
        self.diags.push(Diagnostic::new(kind, span));
        self.diags.last_mut().expect("just pushed")
    }

    /// Finds the structural problems that are decidable from the AST.
    ///
    /// The parser has reported some of these already, and testing again here puts a hand built
    /// AST under the same checks. A document that parsed cleanly does not report twice, since
    /// a parse with errors never reaches `check`.
    fn structural(&mut self, doc: &Document) {
        if doc.stats().nodes == 0 {
            self.push(DiagnosticKind::EmptyDocument, doc.root.span);
        }
        self.walk_structural(&doc.root);
    }

    fn walk_structural(&mut self, node: &Node) {
        let children: Vec<&Node> = node.children().collect();

        // Siblings sharing a name are a warning rather than an error. The rules in both
        // declarations take part in evaluation and the later declaration settles the path
        // type, see `eval::resolve_chain`, so the meaning is unambiguous and this only tells
        // the author that the repetition may or may not have been intended. One path is
        // reported once, at the second declaration, with `related` pointing back at the first.
        let mut by_name: BTreeMap<&str, Vec<&Node>> = BTreeMap::new();
        for child in &children {
            let Some(name) = child.name() else { continue };
            by_name.entry(name).or_default().push(child);
        }
        let dupes: Vec<(&str, Vec<&Node>)> = by_name
            .into_iter()
            .filter(|(_, group)| group.len() > 1)
            .collect();
        for (name, group) in dupes {
            let d = Diagnostic::new(
                DiagnosticKind::DuplicatePath {
                    name: name.into(),
                    count: group.len(),
                },
                group[1].span,
            )
            .with_related(group[0].span);
            self.diags.push(d);
        }

        for child in &children {
            if child.path_type == PathType::Stop && !child.is_leaf() {
                self.push(DiagnosticKind::StopHasChildren, child.span);
            }
        }
        for item in &node.items {
            match item {
                Item::Child(n) => self.walk_structural(n),
                Item::Rule(r) => {
                    if r.op == Op::Explicit && r.targets.is_empty() {
                        self.push(DiagnosticKind::EmptyTargets, r.span);
                    }
                    for t in &r.targets {
                        self.walk_structural(t);
                    }
                }
            }
        }
    }

    fn semantic(&mut self, doc: &Document) {
        let mut sites = Vec::new();
        collect_sites(doc, &doc.root, &mut Vec::new(), &mut sites);

        // Redundancy and conflict, taken one scope at a time.
        self.per_scope(doc, &sites);

        // A dead rule, which a later rule fully covers.
        //
        // A rule already reported as conflicting is not reported as dead, because two
        // diagnostics at one position, one saying it is covered and one saying it fights the
        // rule after it, leave the second with more to say.
        let conflicted: BTreeSet<(u32, u32)> = self
            .diags
            .iter()
            .filter(|d| d.kind == DiagnosticKind::ConflictingRule)
            .map(|d| (d.span.line, d.span.col))
            .collect();
        let dead = dead_rules(doc, &sites);
        for (span, killer) in dead {
            if conflicted.contains(&(span.line, span.col)) {
                continue;
            }
            let d = Diagnostic::new(DiagnosticKind::DeadRule, span).with_related(killer);
            self.diags.push(d);
        }

        // A `>` rule whose target is an isolated path.
        //
        // This judgement uses the same answer as evaluation, through `resolved_type_at`. The
        // earlier version worked from the type the target declaration carried, so when a path
        // resolved to `~` while its declaration read `-` the checker stayed quiet and the
        // evaluator ignored the target anyway.
        for s in &sites {
            for (name, span) in &s.isolated_targets {
                self.push(
                    DiagnosticKind::ExplicitTargetIsolated {
                        target: name.clone(),
                    },
                    *span,
                );
            }
        }
    }

    fn per_scope(&mut self, doc: &Document, sites: &[Site<'_>]) {
        // Several `@` rules in one scope, where the later one fully covers the earlier one.
        let mut scope_of: Vec<(Vec<String>, Vec<usize>)> = Vec::new();
        for (i, s) in sites.iter().enumerate() {
            if s.op != Op::Subtree {
                continue;
            }
            match scope_of.iter().position(|(p, _)| *p == s.scope) {
                Some(k) => scope_of[k].1.push(i),
                None => scope_of.push((s.scope.clone(), vec![i])),
            }
        }

        for (_, idxs) in scope_of {
            let Some((&last, earlier)) = idxs.split_last() else {
                continue;
            };
            let last_color = sites[last].color;
            let last_span = sites[last].span;
            // Rules sharing a color are left to `dead_rules` to call dead, and this handles
            // only the ones whose colors contradict each other, where the author plainly meant
            // one of them and wrote it wrong, which is worth reporting on its own.
            for &i in earlier {
                if sites[i].color != last_color {
                    let d = Diagnostic::new(DiagnosticKind::ConflictingRule, sites[i].span)
                        .with_related(last_span);
                    self.diags.push(d);
                }
            }
        }

        // An `@ c` where an ancestor already propagated `c`, which is redundant.
        for s in sites {
            if s.op != Op::Subtree || s.scope.is_empty() {
                continue;
            }
            let refs: Vec<&str> = s.scope.iter().map(|x| x.as_str()).collect();
            let inherited = inherited_color(doc, &refs);
            if inherited == Some(s.color) {
                self.push(DiagnosticKind::RedundantRule, s.span);
            }
        }
    }
}

/// Returns the color of a path when only the rules in the scopes of its strict ancestors are
/// considered.
///
/// It shares its propagation computation with evaluation, restricting the origins to those
/// above the path.
///
/// # Arguments
///
/// * `doc` - the document being checked
/// * `path` - the path whose inherited color is wanted
fn inherited_color<'a>(doc: &'a Document, path: &[&str]) -> Option<&'a str> {
    let levels = resolve_chain(doc, path);
    let n = path.len();
    let mut best: Option<(u32, &'a str)> = None;
    for i in 0..levels.len().min(n) {
        for rule in levels[i].rules() {
            match rule.op {
                Op::Subtree => {
                    if reaches(&levels, i, n) {
                        consider(&mut best, rule.span.line, &rule.color);
                    }
                }
                Op::Explicit => {
                    if i >= n {
                        continue;
                    }
                    let want = path[i];
                    if !rule.targets.iter().any(|t| t.name() == Some(want)) {
                        continue;
                    }
                    if type_at(&levels, i + 1) == PathType::Isolated {
                        continue;
                    }
                    if reaches(&levels, i + 1, n) {
                        consider(&mut best, rule.span.line, &rule.color);
                    }
                }
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Returns the type a path resolves to, where declarations sharing the name defer to the last
/// one and a path that is not declared counts as recursive.
///
/// # Arguments
///
/// * `doc` - the document being checked
/// * `path` - the path whose type is wanted
fn resolved_type_at(doc: &Document, path: &[String]) -> PathType {
    let refs: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
    type_at(&resolve_chain(doc, &refs), path.len())
}

/// Keeps whichever candidate carries the larger line number.
///
/// # Arguments
///
/// * `best` - the best rule found so far, replaced when this candidate is later
/// * `line` - the line number of the candidate
/// * `color` - the color of the candidate
fn consider<'a>(best: &mut Option<(u32, &'a str)>, line: u32, color: &'a str) {
    if best.map_or(true, |(l, _)| line > l) {
        *best = Some((line, color));
    }
}

/// Where one rule lands in the document.
struct Site<'a> {
    line: u32,
    span: Span,
    op: Op,
    color: &'a str,
    /// The path of the node the rule sits in.
    scope: Vec<String>,
    /// The paths of the origin nodes, which for `@` is `scope` and for `>` is each target.
    ///
    /// Targets that resolve to an isolated path are not among them, because `>` ignores those
    /// and they are recorded separately.
    origins: Vec<Vec<String>>,
    /// The targets a `>` rule ignores, as a name and a position.
    isolated_targets: Vec<(Box<str>, Span)>,
    /// The colors appearing in the rule and where they are, for the vocabulary check.
    colors: Vec<(&'a str, Span)>,
}

fn collect_sites<'a>(
    doc: &Document,
    node: &'a Node,
    path: &mut Vec<String>,
    out: &mut Vec<Site<'a>>,
) {
    for item in &node.items {
        match item {
            Item::Child(n) => {
                let Some(name) = n.name() else { continue };
                path.push(name.to_owned());
                collect_sites(doc, n, path, out);
                path.pop();
            }
            Item::Rule(r) => {
                let mut origins = Vec::new();
                let mut isolated_targets = Vec::new();
                let mut colors = vec![(&*r.color, r.span)];
                match r.op {
                    Op::Subtree => origins.push(path.clone()),
                    Op::Explicit => {
                        for t in &r.targets {
                            let Some(name) = t.name() else { continue };
                            let mut p = path.clone();
                            p.push(name.to_owned());
                            // The test is on the type this path resolves to, not the type that
                            // target declaration carries, because a path declared twice answers
                            // to its later declaration and a `>` rule tints the path itself.
                            // Judging by the declaration would miss one case, where the path
                            // resolves to `~` while the target declaration reads `-`, and then
                            // `>` would inject a color into an isolated subtree.
                            if resolved_type_at(doc, &p) == PathType::Isolated {
                                isolated_targets.push((name.into(), t.span));
                            } else {
                                origins.push(p);
                            }
                        }
                    }
                }
                out.push(Site {
                    line: r.span.line,
                    span: r.span,
                    op: r.op,
                    color: &r.color,
                    scope: path.clone(),
                    origins,
                    isolated_targets,
                    colors: std::mem::take(&mut colors),
                });
                for t in &r.targets {
                    let Some(name) = t.name() else { continue };
                    path.push(name.to_owned());
                    collect_sites(doc, t, path, out);
                    path.pop();
                }
            }
        }
    }
}

/// Returns the rules a later rule fully covers, as the position of the dead rule and the
/// position of the rule that covers it.
///
/// A full cover requires that some origin of the later rule is an ancestor of some origin of
/// the earlier one, or is that same origin, and that a color can travel from that origin all
/// the way down to the earlier one's origin. Every step goes through `reaches`, so whether a
/// `|` or a `~` blocks the way is decided in one place.
///
/// A partial cover does not count, since `delete mods` stays alive after `protect mods/saves`
/// breaks through it.
///
/// # Arguments
///
/// * `doc` - the document being checked
/// * `sites` - every rule of the document together with where it lands
fn dead_rules(doc: &Document, sites: &[Site<'_>]) -> Vec<(Span, Span)> {
    let mut out = Vec::new();
    for (i, r1) in sites.iter().enumerate() {
        if r1.origins.is_empty() {
            continue;
        }
        let mut killer: Option<Span> = None;
        let mut all_covered = true;
        for o1 in &r1.origins {
            let refs: Vec<&str> = o1.iter().map(|x| x.as_str()).collect();
            let levels = resolve_chain(doc, &refs);
            let covered = sites.iter().enumerate().any(|(j, r2)| {
                j != i
                    && r2.line > r1.line
                    && r2.origins.iter().any(|o2| {
                        o2.len() <= o1.len()
                            && o1.starts_with(o2)
                            && reaches(&levels, o2.len(), o1.len())
                    })
            });
            if !covered {
                all_covered = false;
                break;
            }
            if killer.is_none() {
                killer = sites
                    .iter()
                    .enumerate()
                    .filter(|(j, r2)| *j != i && r2.line > r1.line)
                    .filter(|(_, r2)| {
                        r2.origins.iter().any(|o2| {
                            o2.len() <= o1.len()
                                && o1.starts_with(o2)
                                && reaches(&levels, o2.len(), o1.len())
                        })
                    })
                    .map(|(_, r2)| r2.span)
                    .next();
            }
        }
        if all_covered {
            if let Some(k) = killer {
                out.push((r1.span, k));
            }
        }
    }
    out
}
