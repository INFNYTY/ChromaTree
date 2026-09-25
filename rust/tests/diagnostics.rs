//! The Rust side of the diagnostic tests.
//!
//! Every check and the condition that triggers each kind has moved into
//! `conformance/diagnostics.json`, which is what decides whether this implementation follows
//! the specification. Two things cannot move there and stay here, the shape of `Display`,
//! which follows from how Rust works, and the executable form of the rule that the library
//! hands out no natural language.

use chromatree::{parse, DiagnosticKind};

/// `Display for Diagnostic` prints only `code@line:col`, which keeps the library free of
/// natural language.
///
/// The assertions pin that discipline down, since a hand written message holds a space or a
/// non ASCII character while a short code and a position hold neither.
#[test]
fn display_is_machine_readable_ascii() {
    let err = parse("-a\n").expect_err("the parse should fail");
    let text = format!("{}", err[0]);
    assert!(text.is_ascii(), "{text}");
    assert!(text.starts_with("CT"), "{text}");

    let (code, pos) = text.split_once('@').expect("an @ is present");
    assert_eq!(code.len(), 6, "{text}");
    assert_eq!(pos.split(':').count(), 2, "{text}");
    assert!(!text.contains(' '), "no space should appear: {text}");
}

/// A short code keeps its meaning once it is published, so the current values are pinned
/// here and a change breaks this test.
///
/// Agreement with `conformance/codes.json` is covered by `tests/conformance.rs`, and this
/// test guards against the two being changed together.
#[test]
fn codes_are_pinned() {
    assert_eq!(DiagnosticKind::TabIndent.code(), "CT0101");
    assert_eq!(DiagnosticKind::EmptyName.code(), "CT0107");
    assert_eq!(DiagnosticKind::RuleOutsideScope.code(), "CT0112");
    assert_eq!(DiagnosticKind::DeadRule.code(), "CT0202");
    assert_eq!(
        DiagnosticKind::UnknownColor { color: "".into() }.code(),
        "CT0205"
    );
}

/// A severity belongs to the kind and does not vary with the context.
#[test]
fn severity_belongs_to_the_kind() {
    use chromatree::Severity;
    assert_eq!(DiagnosticKind::TabIndent.severity(), Severity::Error);
    assert_eq!(DiagnosticKind::DeadRule.severity(), Severity::Warning);
    assert_eq!(DiagnosticKind::EmptyDocument.severity(), Severity::Warning);
}
