//! Number text object.

use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::range::Range;

use crate::ext::{TextObjectDef, TEXT_OBJECTS};

fn is_digit_or_separator(ch: char) -> bool {
    ch.is_ascii_digit() || ch == '_' || ch == '.'
}

fn is_number_char(ch: char, allow_prefix: bool) -> bool {
    ch.is_ascii_digit() 
        || ch == '_' 
        || ch == '.'
        || ch == 'x' || ch == 'X'  // hex prefix
        || ch == 'b' || ch == 'B'  // binary prefix
        || ch == 'o' || ch == 'O'  // octal prefix
        || (allow_prefix && (ch == '-' || ch == '+'))
        || ('a'..='f').contains(&ch)  // hex digits
        || ('A'..='F').contains(&ch)  // hex digits
        || ch == 'e' || ch == 'E'  // scientific notation
}

fn number_inner(text: RopeSlice, pos: usize) -> Option<Range> {
    let len = text.len_chars();
    if len == 0 {
        return None;
    }
    
    // Check if we're on/near a digit
    let current = text.char(pos);
    if !current.is_ascii_digit() && current != '.' && current != '-' && current != '+' {
        return None;
    }
    
    let mut start = pos;
    let mut end = pos;
    
    // Search backward
    while start > 0 {
        let ch = text.char(start - 1);
        if is_number_char(ch, start == pos) {
            start -= 1;
        } else {
            break;
        }
    }
    
    // Search forward
    while end < len {
        let ch = text.char(end);
        if is_digit_or_separator(ch) || ('a'..='f').contains(&ch) || ('A'..='F').contains(&ch) 
            || ch == 'x' || ch == 'X' || ch == 'b' || ch == 'B' || ch == 'o' || ch == 'O'
            || ch == 'e' || ch == 'E' || ch == '+' || ch == '-'
        {
            end += 1;
        } else {
            break;
        }
    }
    
    if start == end {
        return None;
    }
    
    Some(Range::new(start, end))
}

fn number_around(text: RopeSlice, pos: usize) -> Option<Range> {
    // For numbers, "around" is the same as "inner" - no surrounding delimiters
    number_inner(text, pos)
}

#[distributed_slice(TEXT_OBJECTS)]
static OBJ_NUMBER: TextObjectDef = TextObjectDef {
    name: "number",
    trigger: 'n',
    alt_triggers: &[],
    description: "Select number",
    inner: number_inner,
    around: number_around,
};
