//! Source positions.

/// A region of source text.
///
/// `line` and `col` both count from 1, and `col` counts Unicode scalar values rather than
/// bytes or UTF-16 code units so that it lines up with a character oriented editor. `start`
/// and `end` are a byte range, which is what slicing the source text needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    pub line: u32,
    pub col: u32,
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Builds a span from its four fields.
    ///
    /// # Arguments
    ///
    /// * `line` - the line number, counting from 1
    /// * `col` - the column, counting from 1 in Unicode scalar values
    /// * `start` - the byte offset where the region begins
    /// * `end` - the byte offset where the region ends
    pub fn new(line: u32, col: u32, start: usize, end: usize) -> Self {
        Self {
            line,
            col,
            start,
            end,
        }
    }

    /// Returns a placeholder position that points at no source text, for a hand built AST.
    pub fn dummy() -> Self {
        Self::default()
    }
}
