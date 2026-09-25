//! Evaluation.
//!
//! The core is one pure query, [`Document::color_of`], and it needs no real tree. Under the
//! open world the color of a path follows from the structure and the rules of the document,
//! regardless of whether the real tree contains that path.
//!
//! The rules are in spec §2.4 and §2.5. A color travels down the tree, and each node decides
//! by its own path type whether to accept it and whether to pass it on. For any path the rule
//! that takes effect is the highest numbered rule among those that can reach it.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::ast::{Document, Node, Op, PathType, Rule};
use crate::span::Span;

/// How a color came to be on a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// An `@` rule tinted it directly.
    Subtree,
    /// A `>` rule tinted it directly.
    Explicit,
    /// An ancestor propagated it down.
    Inherited,
}

/// One verdict about a path, which is which path, what color, where in the source and how it
/// was tinted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision<'a> {
    pub path: Vec<String>,
    pub color: &'a str,
    pub span: Span,
    pub via: Via,
}

impl Document {
    /// Returns the color of a path, or `None` when the language has no opinion about it.
    ///
    /// No real tree is needed first, because under the open world the color of a path follows
    /// from the structure and the rules of the document, regardless of whether the real tree
    /// contains that path.
    ///
    /// # Arguments
    ///
    /// * `path` - the segment sequence to look up, where the empty slice is the anonymous root
    pub fn color_of(&self, path: &[&str]) -> Option<&str> {
        color_of_impl(self, path)
    }

    /// Returns every rule that bears on a path, ordered by line number, so that the last one
    /// is the rule in effect.
    ///
    /// A path outside anything the document declares still gets an answer, since propagation
    /// carries on into undeclared descendants.
    ///
    /// # Arguments
    ///
    /// * `path` - the segment sequence to look up
    pub fn explain<'a>(&'a self, path: &[&str]) -> Option<Vec<Decision<'a>>> {
        explain_impl(self, path)
    }

    /// Walks every path the document declares, in document order, giving its path, its type,
    /// its color and the rule that decided the color.
    ///
    /// A path declared more than once produces one entry (§2.8), whose type comes from the
    /// last declaration and whose color takes the rules of every declaration into account.
    ///
    /// Only nodes the document declares are visited, so this says nothing about the open
    /// world. Restricting the question to declared paths is the caller's choice, since the
    /// library answers for any path at all.
    ///
    /// This exists for consumers to compile against, since gathering the path, type, color and
    /// line of every declared node in document order would otherwise take three walks. It
    /// shares one evaluation with [`Document::color_of`].
    pub fn declarations(&self) -> Vec<Declaration<'_>> {
        let mut out = Vec::new();
        let root: [&Node; 1] = [&self.root];
        declare_walk(
            &root,
            PathType::Recursive,
            best_own(&root),
            &mut Vec::new(),
            &mut out,
        );
        out
    }
}

/// One entry from [`Document::declarations`], which is one declared path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration<'a> {
    pub path: Vec<&'a str>,
    /// The path type the last declaration settled on (§2.8).
    pub path_type: PathType,
    /// `None` means ctree has no opinion about this path, which is the colorless case of §3.2.
    pub color: Option<&'a str>,
    /// The line of the rule that took effect, or the line of the last declaration when no
    /// color reaches this path.
    pub span: Span,
}

/// The whole tree form of [`descent`], carrying `best` downward and emitting one entry per
/// declared node.
///
/// It shares one set of carry rules with [`descent`], so the same truth is not worked out
/// twice.
fn declare_walk<'a>(
    nodes: &[&'a Node],
    ty: PathType,
    best: Option<Best<'a>>,
    prefix: &mut Vec<&'a str>,
    out: &mut Vec<Declaration<'a>>,
) {
    // Sibling declarations sharing a name are one path (§2.8), whose type comes from the last
    // declaration and whose children are the union of theirs.
    let mut order: Vec<&'a str> = Vec::new();
    let mut groups: BTreeMap<&'a str, Vec<&'a Node>> = BTreeMap::new();
    for n in nodes {
        for c in n.children() {
            let Some(name) = c.name() else { continue };
            groups
                .entry(name)
                .or_insert_with(|| {
                    order.push(name);
                    Vec::new()
                })
                .push(c);
        }
    }

    for name in order {
        let mut group = groups.remove(name).expect("just inserted");
        // Line number order is declaration order, but sorting pins that down in case the way
        // `items` is built changes later and this quietly follows.
        group.sort_by_key(|n| n.span.line);
        let last = group.last().expect("a group has at least one member");
        let child_ty = last.path_type;

        let inherited = if ty.propagates() && child_ty.accepts() {
            best
        } else {
            None
        };
        let targeted = if child_ty.accepts() {
            best_targeting(nodes, name)
        } else {
            None
        };
        let here = pick(inherited, targeted, best_own(&group));

        prefix.push(name);
        out.push(Declaration {
            path: prefix.clone(),
            path_type: child_ty,
            color: here.map(|b| b.color),
            // Falling back to the declaration line when no rule reaches here, because what
            // the author wants to know is where the path is written.
            span: here.map(|b| b.span).unwrap_or(last.span),
        });
        declare_walk(&group, child_ty, here, prefix, out);
        prefix.pop();
    }
}

pub(crate) fn color_of_impl<'a>(doc: &'a Document, path: &[&str]) -> Option<&'a str> {
    descent(doc, path).map(|b| b.color)
}

/// The rule currently in effect, which is the thing carried down the tree.
///
/// `via` is not recorded, because the carry only cares which rule wins, and how the color got
/// there is the business of [`Decision`].
#[derive(Clone, Copy)]
struct Best<'a> {
    line: u32,
    color: &'a str,
    span: Span,
}

/// Returns the highest numbered `@` rule among the rules in these scopes.
///
/// # Arguments
///
/// * `nodes` - the nodes making up one level of a path, whose scopes are searched together
fn best_own<'a>(nodes: &[&'a Node]) -> Option<Best<'a>> {
    nodes
        .iter()
        .flat_map(|n| n.rules())
        .filter(|r| r.op == Op::Subtree)
        .map(|r| Best {
            line: r.span.line,
            color: &r.color,
            span: r.span,
        })
        .max_by_key(|b| b.line)
}

/// Returns the highest numbered `>` rule targeting `name` among the rules in these scopes.
///
/// # Arguments
///
/// * `parents` - the nodes of the parent level, whose scopes are searched together
/// * `name` - the child name a rule has to target
fn best_targeting<'a>(parents: &[&'a Node], name: &str) -> Option<Best<'a>> {
    parents
        .iter()
        .flat_map(|n| n.rules())
        .filter(|r| r.op == Op::Explicit && r.targets.iter().any(|t| t.name() == Some(name)))
        .map(|r| Best {
            line: r.span.line,
            color: &r.color,
            span: r.span,
        })
        .max_by_key(|b| b.line)
}

/// Returns the highest numbered rule among the three candidates.
///
/// # Arguments
///
/// * `a` - the rule inherited from the parent level
/// * `b` - the rule the parent scope aims at this level
/// * `c` - the rule in this level's own scope
fn pick<'a>(a: Option<Best<'a>>, b: Option<Best<'a>>, c: Option<Best<'a>>) -> Option<Best<'a>> {
    [a, b, c].into_iter().flatten().max_by_key(|x| x.line)
}

/// Walks down one path, carrying the rule in effect the whole way.
///
/// The carry is sound because whether a rule can travel from a parent to a child is all or
/// nothing. The only conditions are that the parent propagates and the child accepts, and both
/// are node properties that do not depend on which rule is in question. Picking the highest
/// numbered rule therefore drops from a set operation to a constant number of comparisons per
/// level, and the cost falls from O(depth * rules) to O(depth + rules along the way).
///
/// This recursion and [`reaches`] are two faces of one meaning, and the corpus and the
/// separately implemented mental model check them against each other.
fn descent<'a>(doc: &'a Document, path: &[&str]) -> Option<Best<'a>> {
    let mut group: Vec<&'a Node> = vec![&doc.root];
    let mut best = best_own(&group);
    // The anonymous root is recursive.
    let mut ty = PathType::Recursive;

    for seg in path {
        let parents = group;
        let mut next: Vec<&'a Node> = parents
            .iter()
            .flat_map(|n| n.children())
            .filter(|c| c.name() == Some(*seg))
            .collect();

        if next.is_empty() {
            // The open world, where an undeclared node counts as recursive and takes the
            // color over to pass it on. When the parent does not propagate, no rule can reach
            // any deeper either.
            if !ty.propagates() {
                return None;
            }
            // What came down from above flows on unchanged.
            ty = PathType::Recursive;
            // Entering the open world, where this level has no declared node and the next one
            // will not either, since an undeclared node carries no children. Leaving the
            // group empty rather than keeping the parent level's nodes matters, because
            // otherwise the next segment would match against the parent's children and find
            // something that is not on the path at all.
            group = Vec::new();
            continue;
        }

        next.sort_by_key(|n| n.span.line);
        let child_ty = next.last().expect("non-empty").path_type;

        let inherited = if ty.propagates() && child_ty.accepts() {
            best
        } else {
            None
        };
        // A `~` node accepts no tint from an ancestor, and a `>` rule travels by exactly that
        // channel, so the target is ignored.
        let targeted = if child_ty.accepts() {
            best_targeting(&parents, seg)
        } else {
            None
        };

        best = pick(inherited, targeted, best_own(&next));
        ty = child_ty;
        group = next;
    }

    best
}

pub(crate) fn explain_impl<'a>(doc: &'a Document, path: &[&str]) -> Option<Vec<Decision<'a>>> {
    let levels = resolve_chain(doc, path);
    let n = path.len();
    let mut out: Vec<(u32, Decision<'a>)> = Vec::new();

    for i in 0..levels.len() {
        // One level can hold several declarations, and every one of their rules has to be
        // visited, which is where "declared twice, both run" lands.
        for rule in levels[i].rules() {
            match rule.op {
                Op::Subtree => {
                    // The origin is the level this rule sits at.
                    if i <= n && reaches(&levels, i, n) {
                        out.push((rule.span.line, mk(rule, path, i, n)));
                    }
                }
                Op::Explicit => {
                    // The origin is the target node, which has to be level i+1 on the path.
                    if i >= n {
                        continue;
                    }
                    let want = path[i];
                    if !rule.targets.iter().any(|t| t.name() == Some(want)) {
                        continue;
                    }
                    // A `~` node accepts no tint from an ancestor, and a `>` rule travels by
                    // exactly that channel, so the target is ignored.
                    //
                    // The test is on the type this path resolves to, not the type of that
                    // target declaration, because a path declared twice answers to its later
                    // declaration and a `>` rule tints the path itself.
                    if type_at(&levels, i + 1) == PathType::Isolated {
                        continue;
                    }
                    if reaches(&levels, i + 1, n) {
                        out.push((rule.span.line, mk(rule, path, i + 1, n)));
                    }
                }
            }
        }
    }

    if out.is_empty() {
        return None;
    }
    out.sort_by_key(|(line, _)| *line);
    Some(out.into_iter().map(|(_, d)| d).collect())
}

fn mk<'a>(rule: &'a Rule, path: &[&str], origin: usize, target: usize) -> Decision<'a> {
    let via = if target > origin {
        Via::Inherited
    } else if rule.op == Op::Subtree {
        Via::Subtree
    } else {
        Via::Explicit
    };
    Decision {
        path: path.iter().map(|s| (*s).to_owned()).collect(),
        color: &rule.color,
        span: rule.span,
        via,
    }
}

/// One level of a queried path, which the same path may declare more than once.
///
/// Declaring a path more than once is not an error. The rules in both declarations take part
/// in evaluation, ordered by line number, which is the same thing as writing two rule blocks
/// for one selector in CSS.
#[derive(Debug)]
pub(crate) struct Level<'a> {
    /// Every declaration at this level sharing the name, ordered by line number. At least one.
    pub nodes: Vec<&'a Node>,
}

impl<'a> Level<'a> {
    /// Returns the path type of this path, which the last declaration settles.
    ///
    /// This is what "the later definition wins" looks like applied to structure. Writing one
    /// path twice means the last writing decides what kind of path it is, which is why §2.5
    /// can no longer say that node declarations take no part in order.
    pub fn path_type(&self) -> PathType {
        self.nodes
            .last()
            .map(|n| n.path_type)
            .unwrap_or(PathType::Recursive)
    }

    /// Returns the rules in every declaration at this level, including its `>` rules and
    /// excluding those in deeper scopes.
    pub fn rules(&self) -> impl Iterator<Item = &'a Rule> + '_ {
        self.nodes.iter().flat_map(|n| n.rules())
    }
}

/// Walks down the queried path and collects the declared nodes found at each level.
///
/// `levels[0]` is the anonymous root and `levels[k]` answers to `path[..k]`. The chain stops
/// where the path enters the open world, and [`type_at`] then treats the remaining levels as
/// recursive.
///
/// # Arguments
///
/// * `doc` - the document being queried
/// * `path` - the segment sequence to resolve
pub(crate) fn resolve_chain<'a>(doc: &'a Document, path: &[&str]) -> Vec<Level<'a>> {
    let mut levels = vec![Level {
        nodes: vec![&doc.root],
    }];
    for seg in path {
        let prev = levels.last().expect("there is at least one level");
        let mut nodes: Vec<&Node> = prev
            .nodes
            .iter()
            .flat_map(|n| n.children())
            .filter(|c| c.name() == Some(*seg))
            .collect();
        if nodes.is_empty() {
            break;
        }
        // `children()` already yields declaration order, and sorting pins down that order
        // equals line number, in case the way `items` is built changes later and this quietly
        // follows.
        nodes.sort_by_key(|n| n.span.line);
        levels.push(Level { nodes });
    }
    levels
}

/// Returns the path type of level `j`, where a level that is not declared has no type and
/// counts as recursive.
///
/// # Arguments
///
/// * `levels` - the levels resolved from the queried path
/// * `j` - the index of the level whose type is wanted
pub(crate) fn type_at(levels: &[Level<'_>], j: usize) -> PathType {
    levels
        .get(j)
        .map(|l| l.path_type())
        .unwrap_or(PathType::Recursive)
}

/// Returns whether a color starting at level `o` can reach level `n`.
///
/// Contract §4 requires every node in between to be a recursive path, the origin to propagate
/// and the destination to accept.
///
/// # Arguments
///
/// * `levels` - the levels resolved from the queried path
/// * `o` - the index of the level the color starts at
/// * `n` - the index of the level the color has to reach
pub(crate) fn reaches(levels: &[Level<'_>], o: usize, n: usize) -> bool {
    if n == o {
        return true;
    }
    if !type_at(levels, o).propagates() {
        return false;
    }
    for j in (o + 1)..n {
        if type_at(levels, j) != PathType::Recursive {
            return false;
        }
    }
    type_at(levels, n).accepts()
}

/// The tinting result for a batch of paths.
///
/// [`Document::color_of`] already answers for any single path, so `Coloring` is not here for
/// speed. It is here because some questions cannot be asked of one path at a time, since
/// finding the roots of a block, finding the exceptions that break through it and counting all
/// compare a set of paths together.
pub struct Coloring<'a> {
    doc: &'a Document,
    paths: Vec<Vec<String>>,
    /// A path that has been looked up, and the color it got.
    ///
    /// Computing all of them up front in `new` was tried and measured slower, because every
    /// path would need two cloned `Vec<String>`, one as the key and one for `paths`, and the
    /// total cost of a single query rose from 2.7ms to 5.7ms. So the colors are computed on
    /// demand and remembered, which pays for the paths asked about and never pays twice for
    /// one of them.
    memo: RefCell<BTreeMap<Vec<String>, Option<&'a str>>>,
}

impl<'a> Coloring<'a> {
    /// Evaluates a batch of paths, computing colors on demand and remembering the ones it has
    /// computed.
    ///
    /// # Arguments
    ///
    /// * `doc` - the document to evaluate against
    /// * `paths` - the paths the batch is made of
    pub fn new(doc: &'a Document, paths: &[Vec<String>]) -> Self {
        Self {
            doc,
            paths: paths.to_vec(),
            memo: RefCell::new(BTreeMap::new()),
        }
    }

    /// Returns the paths the batch is made of.
    pub fn paths(&self) -> &[Vec<String>] {
        &self.paths
    }

    /// Returns the color of a path, reusing an earlier answer and computing one to store when
    /// there is none.
    ///
    /// # Arguments
    ///
    /// * `path` - the segment sequence to look up
    fn lookup(&self, path: &[&str]) -> Option<&'a str> {
        let key: Vec<String> = path.iter().map(|s| (*s).to_owned()).collect();
        if let Some(c) = self.memo.borrow().get(&key) {
            return *c;
        }
        let c = color_of_impl(self.doc, path);
        self.memo.borrow_mut().insert(key, c);
        c
    }

    /// Returns the color of a path, or `None` when the language has no opinion about it.
    ///
    /// # Arguments
    ///
    /// * `path` - the segment sequence to look up
    pub fn color_of(&self, path: &[&str]) -> Option<&'a str> {
        color_of_impl(self.doc, path)
    }

    /// Returns the paths in the batch whose color is `color`.
    ///
    /// # Arguments
    ///
    /// * `color` - the color to select on
    pub fn paths_with(&self, color: &str) -> Vec<Vec<String>> {
        self.paths
            .iter()
            .filter(|p| self.lookup(&as_refs(p)) == Some(color))
            .cloned()
            .collect()
    }

    /// Returns the paths whose color is `color` and whose parent path's color is not.
    ///
    /// This answers which subtrees are `color` throughout. When all of `mods` is `color`, the
    /// answer holds `mods` alone rather than every level beneath it.
    ///
    /// The parent's color is asked for structurally and the parent path need not be in the
    /// batch, since otherwise a caller passing only leaf paths would get the wrong answer.
    ///
    /// # Arguments
    ///
    /// * `color` - the color to select on
    pub fn roots_of(&self, color: &str) -> Vec<Decision<'a>> {
        let mut out = Vec::new();
        for p in &self.paths {
            let segs = as_refs(p);
            let Some(c) = self.lookup(&segs) else {
                continue;
            };
            if c != color || segs.is_empty() {
                continue;
            }
            let parent = &segs[..segs.len() - 1];
            if self.lookup(parent) == Some(color) {
                continue;
            }
            out.push(self.decide(&segs, c));
        }
        out
    }

    /// Returns the paths under `root` whose color differs from the color of `root`, which is
    /// what breaks through the block.
    ///
    /// # Arguments
    ///
    /// * `root` - the path the subtree starts at
    pub fn exceptions_within(&self, root: &[&str]) -> Vec<Decision<'a>> {
        let base = self.lookup(root);
        let mut out = Vec::new();
        for p in &self.paths {
            let segs = as_refs(p);
            if segs.len() <= root.len() || !segs.starts_with(root) {
                continue;
            }
            let Some(c) = self.lookup(&segs) else {
                continue;
            };
            if Some(c) != base {
                out.push(self.decide(&segs, c));
            }
        }
        out
    }

    /// Returns the number of paths of each color in the batch, leaving out the paths that have
    /// no color.
    pub fn counts(&self) -> BTreeMap<&'a str, usize> {
        let mut m = BTreeMap::new();
        for p in &self.paths {
            if let Some(c) = self.lookup(&as_refs(p)) {
                *m.entry(c).or_insert(0) += 1;
            }
        }
        m
    }

    /// Returns the nodes the document declares that the batch does not cover.
    ///
    /// The test is whether the batch holds something equal to a node or falling under it, so a
    /// caller passing only leaf paths is not flooded with middle directories. Failing to match
    /// is entirely normal under the open world, and whether to say anything is the caller's
    /// decision.
    pub fn unmatched(&self) -> Vec<Decision<'a>> {
        let mut out = Vec::new();
        for (segs, span) in self.doc.declared() {
            let covered = self
                .paths
                .iter()
                .any(|p| p.len() >= segs.len() && as_refs(p).starts_with(&segs));
            if covered {
                continue;
            }
            let color = self.lookup(&segs);
            out.push(Decision {
                path: segs.iter().map(|s| (*s).to_owned()).collect(),
                color: color.unwrap_or(""),
                span,
                via: Via::Inherited,
            });
        }
        out
    }

    /// Returns every rule that bears on a path, ordered by line number, answering for a path
    /// outside the batch as well.
    ///
    /// # Arguments
    ///
    /// * `path` - the segment sequence to look up
    pub fn explain(&self, path: &[&str]) -> Option<Vec<Decision<'a>>> {
        explain_impl(self.doc, path)
    }

    fn decide(&self, segs: &[&str], color: &'a str) -> Decision<'a> {
        let span = explain_impl(self.doc, segs)
            .and_then(|v| v.into_iter().next_back())
            .map(|d| d.span)
            .unwrap_or_default();
        Decision {
            path: segs.iter().map(|s| (*s).to_owned()).collect(),
            color,
            span,
            via: Via::Inherited,
        }
    }
}

/// Evaluates a batch of paths.
///
/// # Arguments
///
/// * `doc` - the document to evaluate against
/// * `paths` - the paths the batch is made of
pub fn evaluate<'a>(doc: &'a Document, paths: &[Vec<String>]) -> Coloring<'a> {
    Coloring::new(doc, paths)
}

pub(crate) fn as_refs(p: &[String]) -> Vec<&str> {
    p.iter().map(|s| s.as_str()).collect()
}
