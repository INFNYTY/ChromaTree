//! Property tests.
//!
//! The library is pure, so these tests need no temporary directory, no environment variable
//! and no cleanup, which is the largest advantage it has over modules of the same kind.

use chromatree::{parse, Coloring, Document, Item, Node, Op, PathType, Rule, Span, VERSION};
use proptest::prelude::*;

mod common;
use common::{probe_paths, walk_model};

fn rule(op: Op, color: &str) -> Rule {
    Rule {
        op,
        color: color.into(),
        span: Span::default(),
        targets: Vec::new(),
    }
}

fn node(name: &str, path_type: PathType, items: Vec<Item>) -> Node {
    Node {
        name: Some(name.into()),
        path_type,
        span: Span::default(),
        items,
    }
}

fn path_type() -> impl Strategy<Value = PathType> {
    prop_oneof![
        Just(PathType::Recursive),
        Just(PathType::Stop),
        Just(PathType::Isolated),
    ]
}

fn color() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("red"), Just("blue"), Just("green")]
}

/// A stop path carries no child declaration and no rule, per spec §4.2, which the generator
/// has to respect to produce a valid document.
///
/// # Arguments
///
/// * `pt` - the path type deciding whether the items are kept
/// * `items` - the items declared inside the node
fn with_scope(pt: PathType, items: Vec<Item>) -> Vec<Item> {
    if pt == PathType::Stop {
        Vec::new()
    } else {
        items
    }
}

/// A document of fixed shape and random attributes:
///
/// ```text
/// - a            (random type)
///     @ <ca>
///     - b        (random type)
///         @ <cb>
///         - d
/// - c            (random type)
///     @ <cc>
/// ```
///
/// The shape is fixed to keep the structure valid, with no repeated name among siblings and
/// no skipped indentation level, and the path types and colors are random, so that every
/// branch of the propagation logic is taken.
fn arb_doc() -> impl Strategy<Value = Document> {
    (
        path_type(),
        path_type(),
        path_type(),
        color(),
        color(),
        color(),
    )
        .prop_map(|(ta, tb, tc, ca, cb, cc)| {
            let d = node("d", PathType::Recursive, vec![]);
            let b = node(
                "b",
                tb,
                with_scope(tb, vec![Item::Rule(rule(Op::Subtree, cb)), Item::Child(d)]),
            );
            let a = node(
                "a",
                ta,
                with_scope(ta, vec![Item::Rule(rule(Op::Subtree, ca)), Item::Child(b)]),
            );
            let c = node(
                "c",
                tc,
                with_scope(tc, vec![Item::Rule(rule(Op::Subtree, cc))]),
            );

            let mut root = Node::anonymous_root();
            root.items = vec![Item::Child(a), Item::Child(c)];
            Document {
                version: VERSION,
                root,
            }
        })
}

fn arb_path() -> impl Strategy<Value = Vec<&'static str>> {
    prop_oneof![
        Just(vec![]),
        Just(vec!["a"]),
        Just(vec!["a", "b"]),
        Just(vec!["a", "b", "d"]),
        Just(vec!["a", "b", "d", "deep"]),
        Just(vec!["a", "undeclared"]),
        Just(vec!["c"]),
        Just(vec!["nowhere", "x"]),
    ]
}

fn parse_or_panic(src: &str) -> Document {
    parse(src).unwrap_or_else(|e| panic!("the document should parse:\n{src}\n{e:?}"))
}

proptest! {
    /// Writing back is idempotent, so writing, parsing and writing again gives the same text
    /// character for character.
    #[test]
    fn to_source_is_idempotent(doc in arb_doc()) {
        let src = doc.to_source();
        let once = parse_or_panic(&src).to_source();
        let twice = parse_or_panic(&once).to_source();
        prop_assert_eq!(once, twice);
    }

    /// The written source has to be accepted by the parser, which must not print anything it
    /// would not read itself.
    #[test]
    fn to_source_always_reparses(doc in arb_doc()) {
        let src = doc.to_source();
        let back = parse(&src);
        prop_assert!(
            back.is_ok(),
            "writing back produced a parse failure:\n{}\n{:?}",
            src,
            back.err()
        );
    }

    /// Two evaluations of the same input agree, which guards against an iteration order
    /// leaking out of the internals.
    #[test]
    fn evaluation_is_deterministic(doc in arb_doc(), path in arb_path()) {
        let d = parse_or_panic(&doc.to_source());
        prop_assert_eq!(d.color_of(&path), d.color_of(&path));
    }

    /// Adding paths unrelated to the query changes the color of no path already present.
    #[test]
    fn adding_unrelated_paths_changes_nothing(doc in arb_doc(), path in arb_path()) {
        let d = parse_or_panic(&doc.to_source());
        let before = d.color_of(&path);

        let mut paths: Vec<Vec<String>> = vec![path.iter().map(|s| s.to_string()).collect()];
        paths.push(vec!["totally".into(), "unrelated".into()]);
        let c = Coloring::new(&d, &paths);

        prop_assert_eq!(c.color_of(&path), before);
    }

    /// `roots_of` agrees with `color_of`, since every root it reports carries that color and
    /// its parent carries a different one.
    #[test]
    fn roots_of_agrees_with_color_of(doc in arb_doc()) {
        let d = parse_or_panic(&doc.to_source());
        let all: Vec<Vec<String>> = [
            vec!["a"], vec!["a", "b"], vec!["a", "b", "d"],
            vec!["a", "b", "d", "deep"], vec!["c"],
        ]
        .iter()
        .map(|p| p.iter().map(|s| s.to_string()).collect())
        .collect();
        let c = Coloring::new(&d, &all);

        for col in ["red", "blue", "green"] {
            for r in c.roots_of(col) {
                let segs: Vec<&str> = r.path.iter().map(|s| s.as_str()).collect();
                prop_assert_eq!(
                    c.color_of(&segs),
                    Some(col),
                    "a root does not carry the color of its block"
                );
                if segs.len() > 1 {
                    let parent = &segs[..segs.len() - 1];
                    prop_assert_ne!(
                        c.color_of(parent),
                        Some(col),
                        "the parent carries the same color, so this is not a root"
                    );
                }
            }
        }
    }

    /// An isolated subtree ignores whatever rules are written outside it, which is the
    /// shielding that `~` provides.
    ///
    /// This goes straight at the isolation semantics and is the easiest place to get them
    /// wrong, since it checks both that the `@` of the `~` node itself travels inwards and
    /// that no color from outside gets in.
    #[test]
    fn an_isolated_subtree_ignores_everything_outside(
        outer in prop::collection::vec(color(), 0..4),
        inner in color(),
    ) {
        let mut a_items: Vec<Item> = outer.iter().map(|c| Item::Rule(rule(Op::Subtree, c))).collect();
        let shielded = node(
            "shielded",
            PathType::Isolated,
            vec![
                Item::Rule(rule(Op::Subtree, inner)),
                Item::Child(node("child", PathType::Recursive, vec![])),
            ],
        );
        a_items.push(Item::Child(shielded));
        let a = node("a", PathType::Recursive, a_items);

        let mut root = Node::anonymous_root();
        // One more rule on top, so that colors really are being pushed this way
        root.items = vec![Item::Rule(rule(Op::Subtree, "outermost")), Item::Child(a)];
        let doc = Document { version: VERSION, root };

        let d = parse_or_panic(&doc.to_source());
        prop_assert_eq!(d.color_of(&["a", "shielded"]), Some(inner));
        prop_assert_eq!(d.color_of(&["a", "shielded", "child"]), Some(inner));
        // The `@` of the `~` node keeps travelling down to undeclared descendants
        prop_assert_eq!(d.color_of(&["a", "shielded", "child", "deep"]), Some(inner));
    }

    /// A stop path is a terminus, so its children take no color from this direction.
    #[test]
    fn a_stop_path_colors_nothing_below_it(outer in color(), below in arb_path()) {
        let a = node(
            "a",
            PathType::Recursive,
            vec![Item::Child(node("stop", PathType::Stop, vec![]))],
        );
        let mut root = Node::anonymous_root();
        root.items = vec![Item::Rule(rule(Op::Subtree, outer)), Item::Child(a)];
        let doc = Document { version: VERSION, root };

        let d = parse_or_panic(&doc.to_source());
        prop_assert_eq!(d.color_of(&["a", "stop"]), Some(outer));
        // `["a", "stop", ...]` carries no color however many levels follow
        if !below.is_empty() {
            let mut p = vec!["a", "stop"];
            p.extend(below.iter().copied());
            prop_assert_eq!(d.color_of(&p), None);
        }
    }
}

proptest! {
    /// The mental model and evaluation have to give the same answer.
    ///
    /// These are two independent implementations, where `eval` decides reachability by
    /// arithmetic on indices while this side walks down in earnest. Anywhere they diverge,
    /// either `reaches` is wrong or the model described in §2.9 is not exact.
    #[test]
    fn the_imperative_walk_agrees_with_evaluation(doc in arb_doc()) {
        let d = parse_or_panic(&doc.to_source());
        let probe = probe_paths(&d);
        prop_assume!(!probe.is_empty());
        let walked = walk_model(&d, &probe);
        for p in &probe {
            let refs: Vec<&str> = p.iter().map(|s| s.as_str()).collect();
            let want = walked.get(p).cloned().flatten();
            prop_assert_eq!(
                d.color_of(&refs).map(str::to_owned),
                want,
                "the two implementations diverge on path {:?}: evaluation {:?}, walk {:?}",
                p,
                d.color_of(&refs),
                walked.get(p),
            );
        }
    }
}
