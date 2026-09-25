//! The syntax tree.

use std::collections::BTreeSet;

use crate::span::Span;

/// The language version, which the crate version in `Cargo.toml` tracks.
pub const VERSION: Version = Version { major: 0, minor: 3 };

/// A language version, as a major and a minor number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

/// How a node treats the tint reaching it from its ancestors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PathType {
    /// `-`, which accepts the tint from an ancestor and propagates it.
    Recursive,
    /// `|`, which accepts the tint from an ancestor and stops there.
    Stop,
    /// `~`, which refuses the tint from an ancestor and starts a tint of its own.
    Isolated,
}

impl PathType {
    /// Returns whether this path type accepts the tint arriving from an ancestor.
    pub fn accepts(self) -> bool {
        self != Self::Isolated
    }

    /// Returns whether this path type hands its own tint down to its children.
    pub fn propagates(self) -> bool {
        self != Self::Stop
    }
}

/// The tinting operation a rule performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Op {
    /// `@`, which tints the whole subtree of the node the rule sits in.
    Subtree,
    /// `>`, which tints the direct children the rule lists.
    Explicit,
}

/// A tinting tree, which is what a parsed document holds.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Document {
    pub version: Version,
    /// The anonymous root, whose name is `None` and whose path type is recursive.
    pub root: Node,
}

/// One declared node.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Node {
    /// The node name, which is `None` only on the anonymous root.
    pub name: Option<Box<str>>,
    pub path_type: PathType,
    pub span: Span,
    pub items: Vec<Item>,
}

/// One entry in the scope of a node.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Item {
    /// A child node declaration.
    Child(Node),
    Rule(Rule),
}

/// One tinting rule.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rule {
    pub op: Op,
    pub color: Box<str>,
    pub span: Span,
    /// The targets of a `>` rule, empty for an `@` rule.
    ///
    /// A target node is held by this rule and is also a child of the node the rule sits in,
    /// so [`Node::children`] lists it as well and no node is stored twice.
    pub targets: Vec<Node>,
}

impl Document {
    /// Returns an empty document whose root is anonymous.
    pub fn new() -> Self {
        Self {
            version: VERSION,
            root: Node {
                name: None,
                path_type: PathType::Recursive,
                span: Span::dummy(),
                items: Vec::new(),
            },
        }
    }

    /// Returns every node this document declares, in declaration order, as a segment
    /// sequence and a position.
    ///
    /// The anonymous root is left out because it has no name.
    pub fn declared(&self) -> impl Iterator<Item = (Vec<&str>, Span)> + '_ {
        let mut out = Vec::new();
        collect_declared(&self.root, &mut Vec::new(), &mut out);
        out.into_iter()
    }

    /// Returns the number of declared nodes and rules, and the colors they mention.
    pub fn stats(&self) -> Stats {
        let mut st = Stats {
            nodes: 0,
            rules: 0,
            colors: BTreeSet::new(),
        };
        walk_stats(&self.root, &mut st);
        st
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// The result of [`Document::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    /// The number of declared nodes, not counting the anonymous root.
    pub nodes: usize,
    pub rules: usize,
    /// The colors appearing in the document, deduplicated and ordered.
    pub colors: BTreeSet<Box<str>>,
}

impl Node {
    /// Returns an anonymous root.
    pub fn anonymous_root() -> Self {
        Self {
            name: None,
            path_type: PathType::Recursive,
            span: Span::dummy(),
            items: Vec::new(),
        }
    }

    /// Returns the name of this node, which the anonymous root does not have.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns every child of this node, both the directly declared children and the
    /// targets of its `>` rules.
    pub fn children(&self) -> impl Iterator<Item = &Node> {
        self.items.iter().flat_map(|item| match item {
            Item::Child(n) => std::slice::from_ref(n),
            Item::Rule(r) => r.targets.as_slice(),
        })
    }

    /// Returns the direct child with the given name, or `None` if there is none.
    ///
    /// # Arguments
    ///
    /// * `name` - the node name to match
    ///
    /// Sibling declarations that share a name are a warning rather than an error, see
    /// [`crate::check`], and in that case this returns the first of them.
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children().find(|n| n.name() == Some(name))
    }

    /// Returns every rule in the scope of this node, including its `>` rules and excluding
    /// the rules in the scopes of its children.
    pub fn rules(&self) -> impl Iterator<Item = &Rule> {
        self.items.iter().filter_map(|item| match item {
            Item::Rule(r) => Some(r),
            Item::Child(_) => None,
        })
    }

    /// Returns whether this node has neither a child declaration nor a rule.
    pub fn is_leaf(&self) -> bool {
        self.items.is_empty()
    }
}

fn collect_declared<'a>(
    node: &'a Node,
    prefix: &mut Vec<&'a str>,
    out: &mut Vec<(Vec<&'a str>, Span)>,
) {
    for item in &node.items {
        match item {
            Item::Child(n) => push_declared(n, prefix, out),
            Item::Rule(r) => {
                for t in &r.targets {
                    push_declared(t, prefix, out);
                }
            }
        }
    }
}

fn push_declared<'a>(
    node: &'a Node,
    prefix: &mut Vec<&'a str>,
    out: &mut Vec<(Vec<&'a str>, Span)>,
) {
    let Some(name) = node.name() else { return };
    prefix.push(name);
    out.push((prefix.clone(), node.span));
    collect_declared(node, prefix, out);
    prefix.pop();
}

fn walk_stats(node: &Node, st: &mut Stats) {
    for item in &node.items {
        match item {
            Item::Child(n) => {
                st.nodes += 1;
                walk_stats(n, st);
            }
            Item::Rule(r) => {
                st.rules += 1;
                st.colors.insert(r.color.clone());
                for t in &r.targets {
                    st.nodes += 1;
                    walk_stats(t, st);
                }
            }
        }
    }
}
