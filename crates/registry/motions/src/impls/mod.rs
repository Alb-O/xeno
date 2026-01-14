/// Character-level motions (left, right).
pub(crate) mod basic;
/// Document-level motions (start, end).
pub(crate) mod document;
/// Line-based motions (up, down, first/last line).
pub(crate) mod line;
/// Paragraph-based motions (next/prev paragraph boundary).
pub(crate) mod paragraph;
/// Word-based motions (word, WORD, word-end).
pub(crate) mod word;
