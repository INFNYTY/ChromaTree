//! The Rust side of the evaluation tests.
//!
//! The semantic cases have moved into `conformance/`, which every language runs and which
//! decides whether an implementation follows the specification. What stays here cannot move,
//! namely assertions about Rust types and API that only this implementation offers.
//!
//! A new case should raise one question first. Does the specification require it, in which
//! case it belongs in `conformance/`, or is it particular to this implementation, in which
//! case it belongs here?

use chromatree::{parse, Document, Node, PathType};

fn doc(src: &str) -> Document {
    parse(src).unwrap_or_else(|d| panic!("the document should parse: {d:?}"))
}

/// `Stats` is a convenience API of this implementation and the specification does not ask
/// for it, though the question of which colors a document mentions is neutral and a caller
/// uses the answer to check a vocabulary.
#[test]
fn stats_reports_counts_and_colors() {
    let d = doc("- root\n    @ default\n    > special\n        | fileA\n");
    let st = d.stats();
    assert_eq!(st.rules, 2);
    assert_eq!(st.nodes, 2); // root and fileA
    assert_eq!(
        st.colors.iter().map(|c| &**c).collect::<Vec<_>>(),
        vec!["default", "special"]
    );
}

/// The shape of the anonymous root is a convention of the Rust types, where the name is
/// `None`, the path type is recursive and the empty path stands for it.
#[test]
fn anonymous_root_has_no_name_and_is_recursive() {
    let d = doc("- a\n");
    assert_eq!(d.root.path_type, PathType::Recursive);
    assert_eq!(d.root.name(), None);
    assert_eq!(d.color_of(&[]), None);
}

/// `declared()` hands out segment sequences, which is a type choice of this implementation,
/// while the semantics, meaning which nodes the document declares, are neutral.
#[test]
fn declared_gives_segment_sequences() {
    let d = doc("- a\n    - b\n");
    let got: Vec<String> = d.declared().map(|(segs, _)| segs.join("/")).collect();
    assert_eq!(got, vec!["a", "a/b"]);
}

/// `Decision::path` is an owned `Vec<String>`, since the queried path comes from the caller
/// and is not tied to the lifetime of the document.
#[test]
fn decision_paths_are_owned() {
    let d = doc("- mods\n    @ delete\n");
    let chain = d.explain(&["mods"]).expect("a rule reaches it");
    let owned: Vec<String> = chain[0].path.clone();
    drop(d); // the document goes first and the decision still holds its own path
    assert_eq!(owned, vec!["mods"]);
}

/// The byte range of a `Span` is absolute, so it slices the source text directly.
///
/// This is not in the corpus, since counting offsets in bytes is a type choice of each
/// implementation, but the values have to agree, and the BOM and CRLF are the two places
/// easiest to get wrong, a stripped BOM having to keep the offsets absolute and a `\r`
/// having to count towards them.
#[test]
fn spans_are_absolute_byte_ranges() {
    let node = "- a";
    let rule = "@ red";
    for src in [
        "- a\n    @ red\n",
        "\u{feff}- a\n    @ red\n",
        "- a\r\n    @ red\r\n",
        "\u{feff}- a\r\n    @ red\r\n",
    ] {
        let d = doc(src);
        let (segs, ns) = d.declared().next().expect("a declaration is present");
        assert_eq!(&src[ns.start..ns.end], node, "node slice: {src:?}");

        let chain = d.explain(&segs).expect("a rule reaches it");
        let last = chain.last().expect("the chain is not empty");
        assert_eq!(
            &src[last.span.start..last.span.end],
            rule,
            "rule slice: {src:?}"
        );
    }
}

/// A path declared more than once stays two nodes in the AST rather than being merged into
/// one.
///
/// This is a type choice of this implementation, since the corpus cannot express what the
/// tree looks like inside, and it has an observable consequence in that `to_source()` writes
/// both declarations back. A parser that merged them would lose something on a round trip
/// without saying so.
#[test]
fn duplicate_declarations_stay_separate_in_the_ast() {
    let src = "- root
    ~ saves
        @ keep
    > protect
        - saves
";
    let d = doc(src);

    let root = d.root.child("root").expect("root is present");
    let saves: Vec<&Node> = root
        .children()
        .filter(|c| c.name() == Some("saves"))
        .collect();
    assert_eq!(saves.len(), 2, "two declarations mean two nodes");
    assert_eq!(saves[0].path_type, PathType::Isolated);
    assert_eq!(saves[1].path_type, PathType::Recursive);

    // Nothing is lost on the way back, since the `~` and the `-` declaration are both there.
    let back = d.to_source();
    assert_eq!(back.matches("~ saves").count(), 1, "{back}");
    assert_eq!(back.matches("- saves").count(), 1, "{back}");
}

/// `Node::child` returns only the first node with a given name, which makes it unusable for
/// path resolution.
///
/// Path resolution goes through `resolve_chain`, which takes in every sibling of that name,
/// as the test above and the `duplicate/*` cases in `conformance/` show. This test pins the
/// difference down so that nobody reaches for `child` to look up a path later on.
#[test]
fn node_child_is_not_path_resolution() {
    let d = doc("- root
    ~ first
        @ keep
    - first
        @ later
");
    let root = d.root.child("root").expect("root is present");

    // `child` sees only the first one
    assert_eq!(
        root.child("first").map(|n| n.path_type),
        Some(PathType::Isolated)
    );
    // Evaluation sees both, and the later one wins
    assert_eq!(d.color_of(&["root", "first"]), Some("later"));
    let chain: Vec<&str> = d
        .explain(&["root", "first"])
        .expect("a rule reaches it")
        .iter()
        .map(|x| x.color)
        .collect();
    assert_eq!(chain, vec!["keep", "later"], "both rules reach this path");
}

/// The colors `declarations()` hands out agree with `color_of` one by one.
///
/// It is another way of reading the same evaluation, and this test pins that invariant down.
#[test]
fn declarations_agree_with_color_of() {
    for src in [
        "- root\n    @ default\n    > special\n        | fileA\n",
        "- mods\n    @ delete\n    > protect\n        - saves\n- obsolete\n    @ delete\n",
        "- root\n    ~ saves\n        @ keep\n    > protect\n        - saves\n",
        "- a\n    - b\n        @ blue\n    @ red\n",
        "@ everything\n- a\n    - b\n",
    ] {
        let d = doc(src);
        let decls = d.declarations();
        assert!(!decls.is_empty(), "{src:?}");
        for decl in &decls {
            assert_eq!(
                d.color_of(&decl.path),
                decl.color,
                "{src:?} -> {:?}",
                decl.path
            );
        }
    }
}

/// A path declared more than once yields one entry, whose path type comes from the last
/// declaration (§2.8).
#[test]
fn declarations_merge_duplicate_declarations() {
    let d = doc("- root\n    ~ saves\n        @ keep\n    > protect\n        - saves\n");
    let decls = d.declarations();

    let paths: Vec<String> = decls.iter().map(|x| x.path.join("/")).collect();
    assert_eq!(
        paths,
        vec!["root", "root/saves"],
        "one entry per name: {paths:?}"
    );

    let saves = decls
        .iter()
        .find(|x| x.path == ["root", "saves"])
        .expect("root/saves is present");
    assert_eq!(
        saves.path_type,
        PathType::Recursive,
        "the type comes from the last declaration"
    );
    assert_eq!(saves.color, Some("protect"));
    // The source line is the rule that took effect, the `>` on line 4, not a declaration
    assert_eq!(saves.span.line, 4);
}
