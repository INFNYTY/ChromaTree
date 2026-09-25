//! Items shared by the test binaries that need the mental model of §2.9.
//!
//! Everything here goes through the public API and never reuses the internal
//! `resolve_chain` or `reaches`, because a comparison is worth something only when the two
//! sides are computed apart. Nothing here touches the filesystem, so both `props` and
//! `conformance` can use it.

use std::collections::{BTreeMap, BTreeSet};

use chromatree::{Document, Item, Node, Op, PathType, Rule};

/// Returns the path type a path resolves to, walking down segment by segment and taking the
/// last node when a name repeats.
///
/// The internal `resolve_chain` is deliberately not reused, since this is the second
/// implementation that reads the document directly and both sides have to compute through
/// the public API for a comparison to mean anything.
///
/// # Arguments
///
/// * `doc` - the document holding the declarations
/// * `path` - the path to resolve, as a sequence of segments
pub fn type_of(doc: &Document, path: &[String]) -> PathType {
    let mut node: &Node = &doc.root;
    let mut ty = doc.root.path_type;
    for seg in path {
        let mut found: Option<&Node> = None;
        for c in node.children() {
            if c.name() == Some(seg.as_str()) {
                found = Some(c); // no break, the last match wins
            }
        }
        match found {
            Some(n) => {
                node = n;
                ty = n.path_type;
            }
            // Not declared, so the open world makes it recursive.
            None => return PathType::Recursive,
        }
    }
    ty
}

/// Returns every rule together with its origin path, sorted by line number.
///
/// # Arguments
///
/// * `doc` - the document to walk
pub fn rules_in_line_order(doc: &Document) -> Vec<(u32, &Rule, Vec<String>)> {
    fn go<'a>(node: &'a Node, path: &mut Vec<String>, out: &mut Vec<(u32, &'a Rule, Vec<String>)>) {
        for item in &node.items {
            match item {
                Item::Child(c) => {
                    path.push(c.name().unwrap_or("").to_owned());
                    go(c, path, out);
                    path.pop();
                }
                Item::Rule(r) => {
                    out.push((r.span.line, r, path.clone()));
                    for t in &r.targets {
                        path.push(t.name().unwrap_or("").to_owned());
                        go(t, path, out);
                        path.pop();
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    go(&doc.root, &mut Vec::new(), &mut out);
    out.sort_by_key(|(line, _, _)| *line);
    out
}

/// Runs the model of §2.9 directly, treating the document as an imperative depth first
/// traversal that starts at an origin, walks down and writes as it goes, where the last
/// write wins.
///
/// The probe is the set of paths that matter and has to be closed under prefixes, since it
/// doubles as the finite approximation of the real tree.
///
/// # Arguments
///
/// * `doc` - the document to walk
/// * `probe` - the paths to record a color for, closed under prefixes
pub fn walk_model(doc: &Document, probe: &[Vec<String>]) -> BTreeMap<Vec<String>, Option<String>> {
    let mut nodes: BTreeSet<Vec<String>> = BTreeSet::new();
    nodes.insert(Vec::new()); // the anonymous root
    for p in probe {
        for n in 1..=p.len() {
            nodes.insert(p[..n].to_vec());
        }
    }

    let children_of = |p: &[String]| -> Vec<Vec<String>> {
        nodes
            .iter()
            .filter(|q| q.len() == p.len() + 1 && q.starts_with(p))
            .cloned()
            .collect()
    };

    let mut state: BTreeMap<Vec<String>, Option<String>> = BTreeMap::new();

    // Spread downwards from `p`, which has already been written.
    fn spread(
        doc: &Document,
        p: &[String],
        color: &str,
        children_of: &dyn Fn(&[String]) -> Vec<Vec<String>>,
        state: &mut BTreeMap<Vec<String>, Option<String>>,
    ) {
        state.insert(p.to_vec(), Some(color.to_owned()));
        if !type_of(doc, p).propagates() {
            return; // a stop path, written no further than itself
        }
        for c in children_of(p) {
            if type_of(doc, &c).accepts() {
                spread(doc, &c, color, children_of, state);
            }
            // An isolated path does not accept the color, so it is not entered at all.
        }
    }

    for (_, rule, scope) in rules_in_line_order(doc) {
        match rule.op {
            Op::Subtree => {
                // The origin has to be an ancestor of some probe path, or that path itself,
                // since otherwise it reaches nothing we care about. The empty path is the
                // anonymous root and always reaches.
                if nodes.contains(&scope) {
                    spread(doc, &scope, &rule.color, &children_of, &mut state);
                }
            }
            Op::Explicit => {
                for t in &rule.targets {
                    let Some(name) = t.name() else { continue };
                    let mut p = scope.clone();
                    p.push(name.to_owned());
                    if !nodes.contains(&p) {
                        continue;
                    }
                    if !type_of(doc, &p).accepts() {
                        continue; // an isolated target, so the whole rule does not apply
                    }
                    spread(doc, &p, &rule.color, &children_of, &mut state);
                }
            }
        }
    }

    state
}

/// Returns the probe paths, which are every path the document declares plus one further
/// segment below each of them, so that propagation into the open world is covered.
///
/// # Arguments
///
/// * `doc` - the document whose declarations are probed
pub fn probe_paths(doc: &Document) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    for (segs, _) in doc.declared() {
        let mut p: Vec<String> = segs.iter().map(|s| (*s).to_owned()).collect();
        out.push(p.clone());
        p.push("__probe".to_owned());
        out.push(p);
    }
    out
}
