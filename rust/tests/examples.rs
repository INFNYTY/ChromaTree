//! `examples/*.ctree` is a repository asset rather than part of the implementation, and it
//! has to stay valid and live up to its name.
//!
//! Its behaviour is already covered by `conformance/eval.json`, where the case refers to the
//! file with `source_file` instead of carrying a second copy of it, so the two checks left
//! here concern the file itself.
//!
//! `include_str!` is used rather than a directory read, which settles the content at compile
//! time and keeps the tests free of IO, since the `disallowed_methods` entry in `clippy.toml`
//! applies to `--all-targets` as well. The cost is one line here for each new example, which
//! doubles as a reminder.

use chromatree::{check, parse};

const PERMISSIONS: &str = include_str!("../../examples/permissions.ctree");

#[test]
fn the_example_parses_and_has_no_errors() {
    let d = parse(PERMISSIONS).unwrap_or_else(|e| panic!("the example should parse: {e:?}"));
    let diags = check(&d);
    let errors: Vec<_> = diags.iter().filter(|x| x.is_error()).collect();
    // Warnings are allowed, since the example is deliberately redundant and the tutorial
    // says so.
    assert!(
        errors.is_empty(),
        "the example should hold no error: {errors:?}"
    );
}

/// The example claims that all five symbols appear, so they all have to appear, a comment
/// that lies being worse than no comment at all.
#[test]
fn the_example_actually_uses_all_five_symbols() {
    for (sym, sample) in [
        ("-", "- root"),
        ("|", "| adminPanel"),
        ("~", "~ systemCache"),
        ("@", "@ default"),
        (">", "> protect"),
    ] {
        assert!(
            PERMISSIONS.contains(sample),
            "the example should contain `{sym}`: {sample}"
        );
    }
}
