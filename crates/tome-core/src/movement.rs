use crate::graphemes::{next_grapheme_boundary, prev_grapheme_boundary};
use crate::range::{Direction, Range};
use ropey::RopeSlice;

/// Word type for word movements (Kakoune style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordType {
    /// A word is alphanumeric characters (and those in extra_word_chars).
    Word,
    /// A WORD is any non-whitespace characters.
    WORD,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn make_range(anchor: usize, new_head: usize, extend: bool) -> Range {
    if extend {
        Range::new(anchor, new_head)
    } else {
        Range::point(new_head)
    }
}

pub fn move_horizontally(
    text: RopeSlice,
    range: Range,
    direction: Direction,
    count: usize,
    extend: bool,
) -> Range {
    let pos = range.head;
    let new_pos = match direction {
        Direction::Forward => {
            let mut p = pos;
            for _ in 0..count {
                p = next_grapheme_boundary(text, p);
            }
            p
        }
        Direction::Backward => {
            let mut p = pos;
            for _ in 0..count {
                p = prev_grapheme_boundary(text, p);
            }
            p
        }
    };

    make_range(range.anchor, new_pos, extend)
}

pub fn move_vertically(
    text: RopeSlice,
    range: Range,
    direction: Direction,
    count: usize,
    extend: bool,
) -> Range {
    let pos = range.head;
    let line = text.char_to_line(pos);
    let line_start = text.line_to_char(line);
    let col = pos - line_start;

    let new_line = match direction {
        Direction::Forward => (line + count).min(text.len_lines().saturating_sub(1)),
        Direction::Backward => line.saturating_sub(count),
    };

    let new_line_start = text.line_to_char(new_line);
    let new_line_len = text.line(new_line).len_chars();
    let line_end_offset = if new_line == text.len_lines().saturating_sub(1) {
        new_line_len
    } else {
        new_line_len.saturating_sub(1)
    };

    let new_col = col.min(line_end_offset);
    let new_pos = new_line_start + new_col;

    make_range(range.anchor, new_pos, extend)
}

pub fn move_to_line_start(text: RopeSlice, range: Range, extend: bool) -> Range {
    let line = text.char_to_line(range.head);
    let line_start = text.line_to_char(line);
    make_range(range.anchor, line_start, extend)
}

pub fn move_to_line_end(text: RopeSlice, range: Range, extend: bool) -> Range {
    let line = text.char_to_line(range.head);
    let line_start = text.line_to_char(line);
    let line_len = text.line(line).len_chars();

    let is_last_line = line == text.len_lines().saturating_sub(1);
    let line_end = if is_last_line {
        line_start + line_len
    } else {
        line_start + line_len.saturating_sub(1)
    };

    make_range(range.anchor, line_end, extend)
}

pub fn move_to_first_nonwhitespace(text: RopeSlice, range: Range, extend: bool) -> Range {
    let line = text.char_to_line(range.head);
    let line_start = text.line_to_char(line);
    let line_text = text.line(line);

    let mut first_non_ws = line_start;
    for (i, ch) in line_text.chars().enumerate() {
        if !ch.is_whitespace() {
            first_non_ws = line_start + i;
            break;
        }
    }

    make_range(range.anchor, first_non_ws, extend)
}

/// Move to next word start (Kakoune's `w` command).
/// Selects the word and following whitespace on the right.
pub fn move_to_next_word_start(
    text: RopeSlice,
    range: Range,
    count: usize,
    word_type: WordType,
    extend: bool,
) -> Range {
    let len = text.len_chars();
    if len == 0 {
        return range;
    }

    let mut pos = range.head;

    for _ in 0..count {
        if pos >= len {
            break;
        }

        let start_char = text.char(pos.min(len.saturating_sub(1)));
        let start_is_word = match word_type {
            WordType::Word => is_word_char(start_char),
            WordType::WORD => !start_char.is_whitespace(),
        };

        // Skip current word/WORD
        while pos < len {
            let c = text.char(pos);
            let is_word = match word_type {
                WordType::Word => is_word_char(c),
                WordType::WORD => !c.is_whitespace(),
            };
            if is_word != start_is_word {
                break;
            }
            pos += 1;
        }

        // Skip whitespace
        while pos < len && text.char(pos).is_whitespace() {
            // For word movement, skip whitespace but also watch for newlines
            let c = text.char(pos);
            if c == '\n' {
                pos += 1;
                break;
            }
            pos += 1;
        }
    }

    make_range(range.anchor, pos.min(len), extend)
}

/// Move to next word end (Kakoune's `e` command).
pub fn move_to_next_word_end(
    text: RopeSlice,
    range: Range,
    count: usize,
    word_type: WordType,
    extend: bool,
) -> Range {
    let len = text.len_chars();
    if len == 0 {
        return range;
    }

    let mut pos = range.head;

    for _ in 0..count {
        // Move at least one position
        if pos < len {
            pos += 1;
        }

        // Skip whitespace
        while pos < len && text.char(pos).is_whitespace() {
            pos += 1;
        }

        if pos >= len {
            break;
        }

        // Move to end of word
        let start_char = text.char(pos);
        let start_is_word = match word_type {
            WordType::Word => is_word_char(start_char),
            WordType::WORD => !start_char.is_whitespace(),
        };

        while pos < len {
            let c = text.char(pos);
            let is_word = match word_type {
                WordType::Word => is_word_char(c),
                WordType::WORD => !c.is_whitespace(),
            };
            if is_word != start_is_word {
                break;
            }
            pos += 1;
        }
    }

    // End position is one before where we stopped (last char of word)
    let end_pos = pos.saturating_sub(1).min(len.saturating_sub(1));

    make_range(range.anchor, end_pos, extend)
}

/// Move to previous word start (Kakoune's `b` command).
pub fn move_to_prev_word_start(
    text: RopeSlice,
    range: Range,
    count: usize,
    word_type: WordType,
    extend: bool,
) -> Range {
    let len = text.len_chars();
    if len == 0 {
        return range;
    }

    let mut pos = range.head;

    for _ in 0..count {
        // Move at least one position back
        if pos > 0 {
            pos -= 1;
        }

        // Skip whitespace going backward
        while pos > 0 && text.char(pos).is_whitespace() {
            pos -= 1;
        }

        if pos == 0 {
            break;
        }

        // Move to start of word
        let start_char = text.char(pos);
        let start_is_word = match word_type {
            WordType::Word => is_word_char(start_char),
            WordType::WORD => !start_char.is_whitespace(),
        };

        while pos > 0 {
            let prev_char = text.char(pos - 1);
            let is_word = match word_type {
                WordType::Word => is_word_char(prev_char),
                WordType::WORD => !prev_char.is_whitespace(),
            };
            if is_word != start_is_word {
                break;
            }
            pos -= 1;
        }
    }

    make_range(range.anchor, pos, extend)
}

/// Move to document start.
pub fn move_to_document_start(_text: RopeSlice, range: Range, extend: bool) -> Range {
    make_range(range.anchor, 0, extend)
}

/// Move to document end.
pub fn move_to_document_end(text: RopeSlice, range: Range, extend: bool) -> Range {
    make_range(range.anchor, text.len_chars(), extend)
}

/// Find character forward (Kakoune's `f` and `t` commands).
pub fn find_char_forward(
    text: RopeSlice,
    range: Range,
    target: char,
    count: usize,
    inclusive: bool,
    extend: bool,
) -> Range {
    let len = text.len_chars();
    let mut pos = range.head + 1;
    let mut found_count = 0;

    while pos < len {
        if text.char(pos) == target {
            found_count += 1;
            if found_count >= count {
                let final_pos = if inclusive { pos } else { pos.saturating_sub(1) };
                return make_range(range.anchor, final_pos, extend);
            }
        }
        pos += 1;
    }

    range
}

/// Find character backward (Kakoune's `alt-f` and `alt-t` commands).
pub fn find_char_backward(
    text: RopeSlice,
    range: Range,
    target: char,
    count: usize,
    inclusive: bool,
    extend: bool,
) -> Range {
    if range.head == 0 {
        return range;
    }

    let mut pos = range.head - 1;
    let mut found_count = 0;

    loop {
        if text.char(pos) == target {
            found_count += 1;
            if found_count >= count {
                let final_pos = if inclusive { pos } else { pos + 1 };
                return make_range(range.anchor, final_pos, extend);
            }
        }
        if pos == 0 {
            break;
        }
        pos -= 1;
    }

    range
}

/// Select a word object (inner or around).
/// Inner: just the word characters
/// Around: word + trailing whitespace (or leading if at end)
pub fn select_word_object(
    text: RopeSlice,
    range: Range,
    word_type: WordType,
    inner: bool,
) -> Range {
    let len = text.len_chars();
    if len == 0 {
        return range;
    }

    let pos = range.head.min(len.saturating_sub(1));

    let is_word = match word_type {
        WordType::Word => is_word_char,
        WordType::WORD => |c: char| !c.is_whitespace(),
    };

    let c = text.char(pos);

    // If we're on whitespace, select the whitespace
    if !is_word(c) {
        let mut start = pos;
        let mut end = pos;

        // Extend backward through whitespace
        while start > 0 && !is_word(text.char(start - 1)) {
            start -= 1;
        }
        // Extend forward through whitespace
        while end + 1 < len && !is_word(text.char(end + 1)) {
            end += 1;
        }

        return Range::new(start, end);
    }

    // Find word boundaries
    let mut start = pos;
    let mut end = pos;

    // Extend backward through word chars
    while start > 0 && is_word(text.char(start - 1)) {
        start -= 1;
    }
    // Extend forward through word chars
    while end + 1 < len && is_word(text.char(end + 1)) {
        end += 1;
    }

    if inner {
        Range::new(start, end)
    } else {
        // Around: include trailing whitespace (or leading if at end of line/file)
        let mut around_end = end;
        while around_end + 1 < len {
            let next_c = text.char(around_end + 1);
            if next_c.is_whitespace() && next_c != '\n' {
                around_end += 1;
            } else {
                break;
            }
        }

        if around_end > end {
            Range::new(start, around_end)
        } else {
            // No trailing space, try leading
            let mut around_start = start;
            while around_start > 0 {
                let prev_c = text.char(around_start - 1);
                if prev_c.is_whitespace() && prev_c != '\n' {
                    around_start -= 1;
                } else {
                    break;
                }
            }
            Range::new(around_start, end)
        }
    }
}

/// Select a surround/paired object (parentheses, braces, quotes, etc).
/// Inner: content between delimiters (exclusive)
/// Around: content including delimiters (inclusive)
pub fn select_surround_object(
    text: RopeSlice,
    range: Range,
    open: char,
    close: char,
    inner: bool,
) -> Option<Range> {
    let len = text.len_chars();
    if len == 0 {
        return None;
    }

    let pos = range.head.min(len.saturating_sub(1));
    let balanced = open != close;

    // Find opening delimiter (search backward)
    let mut open_pos = None;
    let mut depth = 0i32;
    let mut search_pos = pos;

    // First check if we're on a delimiter
    let c = text.char(pos);
    if c == open {
        open_pos = Some(pos);
    } else if c == close && balanced {
        depth = 1;
    }

    if open_pos.is_none() {
        // Search backward for opening
        while search_pos > 0 {
            search_pos -= 1;
            let c = text.char(search_pos);
            if balanced {
                if c == close {
                    depth += 1;
                } else if c == open {
                    if depth == 0 {
                        open_pos = Some(search_pos);
                        break;
                    }
                    depth -= 1;
                }
            } else {
                // Quotes: just find the nearest one
                if c == open {
                    open_pos = Some(search_pos);
                    break;
                }
            }
        }
    }

    let open_pos = open_pos?;

    // Find closing delimiter (search forward from opening)
    let mut close_pos = None;
    let mut depth = 0i32;
    let mut search_pos = open_pos + 1;

    while search_pos < len {
        let c = text.char(search_pos);
        if balanced {
            if c == open {
                depth += 1;
            } else if c == close {
                if depth == 0 {
                    close_pos = Some(search_pos);
                    break;
                }
                depth -= 1;
            }
        } else {
            // Quotes: just find the next one
            if c == close {
                close_pos = Some(search_pos);
                break;
            }
        }
        search_pos += 1;
    }

    let close_pos = close_pos?;

    if inner {
        // Inner: between delimiters (exclusive)
        if close_pos > open_pos + 1 {
            Some(Range::new(open_pos + 1, close_pos - 1))
        } else {
            // Empty content
            Some(Range::point(open_pos + 1))
        }
    } else {
        // Around: including delimiters
        Some(Range::new(open_pos, close_pos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn test_move_forward() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);
        let range = Range::point(0);

        let moved = move_horizontally(slice, range, Direction::Forward, 1, false);
        assert_eq!(moved.head, 1);
    }

    #[test]
    fn test_move_backward() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);
        let range = Range::point(5);

        let moved = move_horizontally(slice, range, Direction::Backward, 2, false);
        assert_eq!(moved.head, 3);
    }

    #[test]
    fn test_move_forward_extend() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);
        let range = Range::point(0);

        let moved = move_horizontally(slice, range, Direction::Forward, 5, true);
        assert_eq!(moved.anchor, 0);
        assert_eq!(moved.head, 5);
    }

    #[test]
    fn test_move_down() {
        let text = Rope::from("hello\nworld\n");
        let slice = text.slice(..);
        let range = Range::point(2);

        let moved = move_vertically(slice, range, Direction::Forward, 1, false);
        assert_eq!(moved.head, 8);
    }

    #[test]
    fn test_move_up() {
        let text = Rope::from("hello\nworld\n");
        let slice = text.slice(..);
        let range = Range::point(8);

        let moved = move_vertically(slice, range, Direction::Backward, 1, false);
        assert_eq!(moved.head, 2);
    }

    #[test]
    fn test_move_to_line_start() {
        let text = Rope::from("hello\nworld\n");
        let slice = text.slice(..);
        let range = Range::point(8);

        let moved = move_to_line_start(slice, range, false);
        assert_eq!(moved.head, 6);
    }

    #[test]
    fn test_move_to_line_end() {
        let text = Rope::from("hello\nworld\n");
        let slice = text.slice(..);
        let range = Range::point(6);

        let moved = move_to_line_end(slice, range, false);
        assert_eq!(moved.head, 11);
    }

    #[test]
    fn test_move_to_first_nonwhitespace() {
        let text = Rope::from("  hello\n");
        let slice = text.slice(..);
        let range = Range::point(0);

        let moved = move_to_first_nonwhitespace(slice, range, false);
        assert_eq!(moved.head, 2);
    }

    #[test]
    fn test_move_to_next_word_start() {
        let text = Rope::from("hello world test");
        let slice = text.slice(..);
        let range = Range::point(0);

        // From 'h', move to 'w'
        let moved = move_to_next_word_start(slice, range, 1, WordType::Word, false);
        assert_eq!(moved.head, 6);

        // From 'w', move to 't'
        let moved2 = move_to_next_word_start(slice, moved, 1, WordType::Word, false);
        assert_eq!(moved2.head, 12);
    }

    #[test]
    fn test_move_to_next_word_start_count() {
        let text = Rope::from("one two three four");
        let slice = text.slice(..);
        let range = Range::point(0);

        // Move 2 words
        let moved = move_to_next_word_start(slice, range, 2, WordType::Word, false);
        assert_eq!(moved.head, 8); // 't' of 'three'
    }

    #[test]
    fn test_move_to_prev_word_start() {
        let text = Rope::from("hello world test");
        let slice = text.slice(..);
        let range = Range::point(12); // at 't' of 'test'

        let moved = move_to_prev_word_start(slice, range, 1, WordType::Word, false);
        assert_eq!(moved.head, 6); // 'w' of 'world'
    }

    #[test]
    fn test_move_to_next_word_end() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);
        let range = Range::point(0);

        let moved = move_to_next_word_end(slice, range, 1, WordType::Word, false);
        assert_eq!(moved.head, 4); // 'o' of 'hello'
    }

    #[test]
    fn test_find_char_forward() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);
        let range = Range::point(0);

        // Find 'o', inclusive
        let moved = find_char_forward(slice, range, 'o', 1, true, false);
        assert_eq!(moved.head, 4);

        // Find 'o', exclusive (t command)
        let moved = find_char_forward(slice, range, 'o', 1, false, false);
        assert_eq!(moved.head, 3);
    }

    #[test]
    fn test_find_char_forward_count() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);
        let range = Range::point(0);

        // Find second 'o'
        let moved = find_char_forward(slice, range, 'o', 2, true, false);
        assert_eq!(moved.head, 7);
    }

    #[test]
    fn test_find_char_backward() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);
        let range = Range::point(10);

        let moved = find_char_backward(slice, range, 'o', 1, true, false);
        assert_eq!(moved.head, 7);
    }

    #[test]
    fn test_document_movement() {
        let text = Rope::from("line1\nline2\nline3");
        let slice = text.slice(..);
        let range = Range::point(7);

        let start = move_to_document_start(slice, range, false);
        assert_eq!(start.head, 0);

        let end = move_to_document_end(slice, range, false);
        assert_eq!(end.head, 17);
    }

    #[test]
    fn test_word_movement_extend() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);
        let range = Range::point(0);

        let moved = move_to_next_word_start(slice, range, 1, WordType::Word, true);
        assert_eq!(moved.anchor, 0);
        assert_eq!(moved.head, 6);
    }

    #[test]
    fn test_select_word_object_inner() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);

        // Cursor on 'e' in hello
        let range = Range::point(1);
        let selected = select_word_object(slice, range, WordType::Word, true);
        assert_eq!(selected.from(), 0);
        assert_eq!(selected.to(), 4); // "hello" is positions 0-4

        // Cursor on 'o' in world
        let range = Range::point(7);
        let selected = select_word_object(slice, range, WordType::Word, true);
        assert_eq!(selected.from(), 6);
        assert_eq!(selected.to(), 10); // "world" is positions 6-10
    }

    #[test]
    fn test_select_word_object_around() {
        let text = Rope::from("hello world");
        let slice = text.slice(..);

        // Cursor on 'e' in hello - around includes trailing space
        let range = Range::point(1);
        let selected = select_word_object(slice, range, WordType::Word, false);
        assert_eq!(selected.from(), 0);
        assert_eq!(selected.to(), 5); // "hello " is positions 0-5
    }

    #[test]
    fn test_select_surround_object_parens() {
        let text = Rope::from("foo(bar)baz");
        let slice = text.slice(..);

        // Cursor inside parens on 'a'
        let range = Range::point(5);

        // Inner: just "bar"
        let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
        assert_eq!(selected.from(), 4);
        assert_eq!(selected.to(), 6); // "bar" is positions 4-6

        // Around: "(bar)"
        let selected = select_surround_object(slice, range, '(', ')', false).unwrap();
        assert_eq!(selected.from(), 3);
        assert_eq!(selected.to(), 7); // "(bar)" is positions 3-7
    }

    #[test]
    fn test_select_surround_object_nested() {
        let text = Rope::from("foo(a(b)c)bar");
        let slice = text.slice(..);

        // Cursor on 'b' inside inner parens
        let range = Range::point(6);
        let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
        assert_eq!(selected.from(), 6);
        assert_eq!(selected.to(), 6); // inner of (b) is just "b"

        // Cursor on 'a' - should get inner of outer parens
        let range = Range::point(4);
        let selected = select_surround_object(slice, range, '(', ')', true).unwrap();
        assert_eq!(selected.from(), 4);
        assert_eq!(selected.to(), 8); // inner of (a(b)c) is "a(b)c"
    }

    #[test]
    fn test_select_surround_object_quotes() {
        let text = Rope::from(r#"say "hello" now"#);
        let slice = text.slice(..);

        // Cursor on 'e' inside quotes
        let range = Range::point(6);

        // Inner: just "hello"
        let selected = select_surround_object(slice, range, '"', '"', true).unwrap();
        assert_eq!(selected.from(), 5);
        assert_eq!(selected.to(), 9); // "hello" is positions 5-9

        // Around: "\"hello\""
        let selected = select_surround_object(slice, range, '"', '"', false).unwrap();
        assert_eq!(selected.from(), 4);
        assert_eq!(selected.to(), 10);
    }
}
