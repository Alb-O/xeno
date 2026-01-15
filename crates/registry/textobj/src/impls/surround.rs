//! Bracket/surround text objects.

use crate::bracket_pair_object;

bracket_pair_object!(parentheses, '(', ')', 'b', &['(', ')']);
bracket_pair_object!(braces, '{', '}', 'B', &['{', '}']);
bracket_pair_object!(brackets, '[', ']', 'r', &['[', ']']);
bracket_pair_object!(angle_brackets, '<', '>', 'a', &['<', '>']);
