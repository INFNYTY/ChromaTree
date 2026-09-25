//! Diagnostics.
//!
//! This module produces no natural language. A diagnostic carries a kind, a position and a
//! severity, and the caller turns those into wording in whichever language it serves. The
//! library does hand out a stable short code for each kind, which logs and issue trackers
//! use as a key rather than as a sentence.

use std::fmt;

use crate::span::Span;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Severity {
    Error,
    Warning,
}

/// The kind of a diagnostic.
///
/// The enum is not marked `#[non_exhaustive]`, because a caller ought to match it
/// exhaustively and a missed variant ought to fail to compile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiagnosticKind {
    // Structural, CT01xx.
    /// A tab appears in the indentation.
    TabIndent,
    /// The base indentation is none of 2, 4 or 8.
    IndentBase { got: usize },
    /// The indentation of a line is not a multiple of the base.
    IndentMixed { indent: usize },
    /// A child is indented more than one level deeper than its parent.
    IndentJump,
    /// A line is none of blank, comment or declaration.
    UnexpectedLine,
    /// No space follows the symbol.
    MissingSeparator,
    /// A declaration line has no name after its symbol.
    EmptyName,
    /// A `|` node carries a child declaration or a `>` rule.
    StopHasChildren,
    /// A `>` rule lists no target at all.
    EmptyTargets,
    /// A tinting rule falls outside the scope of any node, because the block of a `>` rule
    /// admits node declarations only.
    RuleOutsideScope,
    /// The same path is declared more than once.
    ///
    /// This is a warning rather than an error. The rules in both declarations take part in
    /// evaluation, ordered by line number, and the path type comes from the last
    /// declaration. It is the same situation as two rule blocks for one selector in CSS,
    /// so the warning only says that the author may or may not have meant to write it
    /// twice.
    DuplicatePath { name: Box<str>, count: usize },
    /// The document declares no node at all.
    EmptyDocument,

    // Semantic, CT02xx.
    /// The target of a `>` rule is a `~` node, so the rule ignores that target and is
    /// redundant.
    ExplicitTargetIsolated { target: Box<str> },
    /// A rule is fully overridden by a rule with a larger line number, which is the one
    /// recorded in [`Diagnostic::related`].
    DeadRule,
    /// Two `@` rules in one scope give different colors.
    ConflictingRule,
    /// An `@` rule repeats the color an ancestor already propagated with no `~` in between.
    RedundantRule,
    /// A color is absent from the vocabulary the caller supplied.
    UnknownColor { color: Box<str> },
}

impl DiagnosticKind {
    /// Returns the stable short code of this kind, shaped like `CT0107`.
    ///
    /// Once released, a code never changes meaning, and a new diagnostic takes a new code
    /// rather than reusing an old one.
    pub fn code(&self) -> &'static str {
        match self {
            Self::TabIndent => "CT0101",
            Self::IndentBase { .. } => "CT0102",
            Self::IndentMixed { .. } => "CT0103",
            Self::IndentJump => "CT0104",
            Self::UnexpectedLine => "CT0105",
            Self::MissingSeparator => "CT0106",
            Self::EmptyName => "CT0107",
            Self::StopHasChildren => "CT0108",
            Self::EmptyTargets => "CT0109",
            Self::DuplicatePath { .. } => "CT0110",
            Self::EmptyDocument => "CT0111",
            Self::RuleOutsideScope => "CT0112",
            Self::ExplicitTargetIsolated { .. } => "CT0201",
            Self::DeadRule => "CT0202",
            Self::ConflictingRule => "CT0203",
            Self::RedundantRule => "CT0204",
            Self::UnknownColor { .. } => "CT0205",
        }
    }

    /// Returns the severity, which is fixed per kind and does not vary with context.
    pub fn severity(&self) -> Severity {
        match self {
            Self::EmptyDocument
            | Self::DuplicatePath { .. }
            | Self::ExplicitTargetIsolated { .. }
            | Self::DeadRule
            | Self::ConflictingRule
            | Self::RedundantRule => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

/// One reported problem.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagnostic {
    pub severity: Severity,
    pub kind: DiagnosticKind,
    pub span: Span,
    /// Other positions this diagnostic points at, which `DeadRule` uses to name the rule
    /// that overrides it.
    pub related: Vec<Span>,
}

impl Diagnostic {
    /// Builds a diagnostic whose severity follows from its kind.
    ///
    /// # Arguments
    ///
    /// * `kind` - the kind of problem being reported
    /// * `span` - the source position the problem is reported at
    pub fn new(kind: DiagnosticKind, span: Span) -> Self {
        Self {
            severity: kind.severity(),
            kind,
            span,
            related: Vec::new(),
        }
    }

    /// Adds a related position and returns the diagnostic.
    ///
    /// # Arguments
    ///
    /// * `related` - another position this diagnostic points at
    pub fn with_related(mut self, related: Span) -> Self {
        self.related.push(related);
        self
    }

    /// Returns whether the severity is [`Severity::Error`].
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Returns the short code of this diagnostic's kind.
    ///
    /// This is a key rather than wording meant for people, and the readable message is the
    /// caller's to produce. Displaying the diagnostic adds the position, as in
    /// `CT0107@12:5`.
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}:{}", self.code(), self.span.line, self.span.col)
    }
}

/// Returns the name and the stable short code of every diagnostic kind this build knows.
///
/// The table is a cross language contract, because logs and issue trackers rely on it to
/// line up the same problem across implementations, and it is the content of
/// `conformance/codes.json`. Exposing it as a function lets a test keep the two in step.
///
/// The names correspond one to one with the keys of `codes.json`.
pub fn all_diagnostic_codes() -> Vec<(&'static str, &'static str)> {
    use DiagnosticKind::*;
    [
        TabIndent,
        IndentBase { got: 0 },
        IndentMixed { indent: 0 },
        IndentJump,
        UnexpectedLine,
        MissingSeparator,
        EmptyName,
        StopHasChildren,
        EmptyTargets,
        RuleOutsideScope,
        DuplicatePath {
            name: "".into(),
            count: 0,
        },
        EmptyDocument,
        ExplicitTargetIsolated { target: "".into() },
        DeadRule,
        ConflictingRule,
        RedundantRule,
        UnknownColor { color: "".into() },
    ]
    .into_iter()
    .map(|k| {
        let name = match &k {
            TabIndent => "TabIndent",
            IndentBase { .. } => "IndentBase",
            IndentMixed { .. } => "IndentMixed",
            IndentJump => "IndentJump",
            UnexpectedLine => "UnexpectedLine",
            MissingSeparator => "MissingSeparator",
            EmptyName => "EmptyName",
            StopHasChildren => "StopHasChildren",
            EmptyTargets => "EmptyTargets",
            RuleOutsideScope => "RuleOutsideScope",
            DuplicatePath { .. } => "DuplicatePath",
            EmptyDocument => "EmptyDocument",
            ExplicitTargetIsolated { .. } => "ExplicitTargetIsolated",
            DeadRule => "DeadRule",
            ConflictingRule => "ConflictingRule",
            RedundantRule => "RedundantRule",
            UnknownColor { .. } => "UnknownColor",
        };
        (name, k.code())
    })
    .collect()
}
