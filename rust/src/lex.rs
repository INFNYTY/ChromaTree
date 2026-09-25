//! Line level lexing.
//!
//! The specification in §4.1 says a line is one of three things, blank, comment or
//! declaration, and there is no fourth. A line outside those three is always an error and
//! is never skipped without a word, because skipping it silently would let one missing `-`
//! turn into a whole subtree quietly moving somewhere else.

use crate::diag::{Diagnostic, DiagnosticKind};
use crate::span::Span;

/// One of the five symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sym {
    /// `-`
    Dash,
    /// `|`
    Pipe,
    /// `~`
    Tilde,
    /// `>`
    Gt,
    /// `@`
    At,
}

#[derive(Debug)]
pub(crate) enum LineKind {
    Blank,
    Comment,
    Decl {
        sym: Sym,
        name: Box<str>,
    },
    /// A symbol leads the line, but what follows it is malformed.
    Malformed {
        reason: Malformed,
    },
    /// The line does not even lead with a symbol.
    Prose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Malformed {
    /// No space follows the symbol.
    MissingSeparator,
    /// No name follows the symbol.
    EmptyName,
}

#[derive(Debug)]
pub(crate) struct Line {
    /// The number of leading spaces.
    pub indent: usize,
    pub kind: LineKind,
    /// The range from the symbol, or from the first non blank character, to the end of the
    /// line. On a blank line it is the start of the line.
    pub span: Span,
}

/// Cuts source text into lines, classifies each one and reports what it finds.
///
/// # Arguments
///
/// * `src` - the source text of the whole document
pub(crate) fn lex(src: &str) -> (Vec<Line>, Vec<Diagnostic>) {
    let mut lines = Vec::new();
    let mut diags = Vec::new();

    // A UTF-8 byte order mark, which Notepad on Windows writes by default and which
    // therefore cannot be ignored. Keeping it would make the first line look as though it
    // does not begin with a symbol, putting the whole document wrong from its first line.
    //
    // The mark is stripped, but the byte offsets stay absolute, because the caller uses
    // `start` and `end` to locate a position in the original text. The column does not
    // count it, since an editor does not show the mark as a column.
    let (src, base) = match src.strip_prefix('\u{feff}') {
        Some(rest) => (rest, '\u{feff}'.len_utf8()),
        None => (src, 0),
    };

    let mut offset = base;
    for (idx, raw) in src.split_inclusive('\n').enumerate() {
        let number = idx as u32 + 1;
        let text = raw.strip_suffix('\n').unwrap_or(raw);
        // A trailing `\r` means CRLF line endings. Only the one at the end is stripped,
        // because a name is everything before the end of the line kept verbatim, and a `\r`
        // in the middle is one more character of the name.
        let text = text.strip_suffix('\r').unwrap_or(text);

        let (line, mut d) = lex_line(number, offset, text);
        lines.push(line);
        diags.append(&mut d);

        offset += raw.len();
    }
    // `split_inclusive` produces no trailing empty line for input that ends with a newline,
    // which is what is wanted, and produces no line at all for empty input.
    //
    // Only `\n` counts as a newline. A lone `\r` from an old Mac is not treated as one, so
    // such a file is read as a single line. See spec §4.1.
    (lines, diags)
}

fn lex_line(number: u32, offset: usize, text: &str) -> (Line, Vec<Diagnostic>) {
    let mut diags = Vec::new();

    // Leading whitespace.
    let mut indent = 0usize;
    let mut i = 0usize;
    let mut saw_tab = false;
    for (bi, ch) in text.char_indices() {
        match ch {
            ' ' => {
                if !saw_tab {
                    indent += 1;
                }
                i = bi + 1;
            }
            '\t' => {
                saw_tab = true;
                i = bi + 1;
            }
            _ => break,
        }
    }

    let sym_start = i;
    let col = col_of(text, sym_start);
    let span = Span::new(number, col, offset + sym_start, offset + text.len());

    // A tab is reported only on a declaration line. The indentation of a blank or comment
    // line takes no part in the structure, so a tab in front of one has nothing to do with
    // saying the indentation unambiguously, and reporting it would only be noise.
    let tab_diag = |diags: &mut Vec<Diagnostic>| {
        if saw_tab {
            diags.push(Diagnostic::new(
                DiagnosticKind::TabIndent,
                Span::new(number, 1, offset, offset + sym_start),
            ));
        }
    };

    let rest = &text[sym_start..];

    if rest.is_empty() {
        return (
            Line {
                indent,
                kind: LineKind::Blank,
                span,
            },
            diags,
        );
    }

    if rest.starts_with('#') || rest.starts_with("//") {
        return (
            Line {
                indent,
                kind: LineKind::Comment,
                span,
            },
            diags,
        );
    }

    let first = rest.chars().next().expect("non-empty");
    let Some(sym) = sym_of(first) else {
        return (
            Line {
                indent,
                kind: LineKind::Prose,
                span,
            },
            diags,
        );
    };

    // Reaching here means the line is a declaration, even a malformed one, so its
    // indentation now carries meaning and the tab check applies.
    tab_diag(&mut diags);

    let after = &rest[first.len_utf8()..];
    let kind = if after.is_empty() {
        LineKind::Malformed {
            reason: Malformed::EmptyName,
        }
    } else if !after.starts_with(' ') {
        LineKind::Malformed {
            reason: Malformed::MissingSeparator,
        }
    } else {
        let name = after.trim();
        if name.is_empty() {
            LineKind::Malformed {
                reason: Malformed::EmptyName,
            }
        } else {
            LineKind::Decl {
                sym,
                name: name.into(),
            }
        }
    };

    (Line { indent, kind, span }, diags)
}

fn sym_of(ch: char) -> Option<Sym> {
    Some(match ch {
        '-' => Sym::Dash,
        '|' => Sym::Pipe,
        '~' => Sym::Tilde,
        '>' => Sym::Gt,
        '@' => Sym::At,
        _ => return None,
    })
}

/// Returns the column number counting from 1, counted in Unicode scalar values.
///
/// # Arguments
///
/// * `text` - the line the position falls on
/// * `byte` - the byte offset whose column is wanted
fn col_of(text: &str, byte: usize) -> u32 {
    text[..byte].chars().count() as u32 + 1
}
