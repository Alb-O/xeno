//! Default keybindings for normal mode.

use linkme::distributed_slice;

use crate::ext::keybindings::{BindingMode, KEYBINDINGS_NORMAL, KeyBindingDef};
use crate::key::{Key, SpecialKey};

const DEFAULT_PRIORITY: i16 = 100;

macro_rules! bind {
	($name:ident, $key:expr, $action:expr) => {
		#[distributed_slice(KEYBINDINGS_NORMAL)]
		static $name: KeyBindingDef = KeyBindingDef {
			mode: BindingMode::Normal,
			key: $key,
			action: $action,
			priority: DEFAULT_PRIORITY,
		};
	};
}

bind!(KB_H, Key::char('h'), "move_left");
bind!(KB_L, Key::char('l'), "move_right");
bind!(KB_J, Key::char('j'), "move_down_visual");
bind!(KB_K, Key::char('k'), "move_up_visual");
bind!(KB_LEFT, Key::special(SpecialKey::Left), "move_left");
bind!(KB_RIGHT, Key::special(SpecialKey::Right), "move_right");
bind!(KB_DOWN, Key::special(SpecialKey::Down), "move_down_visual");
bind!(KB_UP, Key::special(SpecialKey::Up), "move_up_visual");
bind!(KB_HOME, Key::special(SpecialKey::Home), "move_line_start");
bind!(KB_END, Key::special(SpecialKey::End), "move_line_end");
bind!(
	KB_HOME_CTRL,
	Key::special(SpecialKey::Home).with_ctrl(),
	"document_start"
);
bind!(
	KB_END_CTRL,
	Key::special(SpecialKey::End).with_ctrl(),
	"document_end"
);
bind!(
	KB_PAGE_UP,
	Key::special(SpecialKey::PageUp),
	"scroll_page_up"
);
bind!(
	KB_PAGE_DOWN,
	Key::special(SpecialKey::PageDown),
	"scroll_page_down"
);

bind!(KB_W, Key::char('w'), "next_word_start");
bind!(KB_B, Key::char('b'), "prev_word_start");
bind!(KB_E, Key::char('e'), "next_word_end");

bind!(KB_W_UPPER, Key::char('W'), "next_long_word_start");
bind!(KB_B_UPPER, Key::char('B'), "prev_long_word_start");
bind!(KB_E_UPPER, Key::char('E'), "next_long_word_end");
bind!(KB_W_ALT, Key::alt('w'), "next_long_word_start");
bind!(KB_B_ALT, Key::alt('b'), "prev_long_word_start");
bind!(KB_E_ALT, Key::alt('e'), "next_long_word_end");

bind!(KB_0, Key::char('0'), "move_line_start");
bind!(KB_CARET, Key::char('^'), "move_first_nonblank");
bind!(KB_DOLLAR, Key::char('$'), "move_line_end");
bind!(KB_H_ALT, Key::alt('h'), "move_line_start");
bind!(KB_L_ALT, Key::alt('l'), "move_line_end");

bind!(KB_GG, Key::char('g'), "goto_mode");
bind!(KB_G_UPPER, Key::char('G'), "document_end");

bind!(KB_D, Key::char('d'), "delete");
bind!(KB_D_ALT, Key::alt('d'), "delete_no_yank");
bind!(KB_C, Key::char('c'), "change");
bind!(KB_C_ALT, Key::alt('c'), "change_no_yank");
bind!(KB_Y, Key::char('y'), "yank");
bind!(KB_P, Key::char('p'), "paste_after");
bind!(KB_P_UPPER, Key::char('P'), "paste_before");
bind!(KB_P_ALT, Key::alt('p'), "paste_all_after");
bind!(KB_P_ALT_UPPER, Key::alt('P'), "paste_all_before");

bind!(KB_U, Key::char('u'), "undo");
bind!(KB_U_UPPER, Key::char('U'), "redo");

bind!(KB_I, Key::char('i'), "insert_before");
bind!(KB_A, Key::char('a'), "insert_after");
bind!(KB_I_UPPER, Key::char('I'), "insert_line_start");
bind!(KB_A_UPPER, Key::char('A'), "insert_line_end");
bind!(KB_O, Key::char('o'), "open_below");
bind!(KB_O_UPPER, Key::char('O'), "open_above");
bind!(KB_O_ALT, Key::alt('o'), "add_line_below");
bind!(KB_O_ALT_UPPER, Key::alt('O'), "add_line_above");

bind!(
	KB_ESC,
	Key::special(SpecialKey::Escape),
	"collapse_selection"
);
bind!(KB_SEMI, Key::char(';'), "collapse_selection");
bind!(KB_SEMI_ALT, Key::alt(';'), "flip_selection");
bind!(KB_COLON_ALT, Key::alt(':'), "ensure_forward");
bind!(KB_COMMA, Key::char(','), "keep_primary_selection");
bind!(KB_COMMA_ALT, Key::alt(','), "remove_primary_selection");
bind!(KB_PAREN_CLOSE, Key::char(')'), "rotate_selections_forward");
bind!(KB_PAREN_OPEN, Key::char('('), "rotate_selections_backward");

bind!(KB_X, Key::char('x'), "select_line");
bind!(KB_X_ALT, Key::alt('x'), "trim_to_line");
bind!(KB_PERCENT, Key::char('%'), "select_all");

bind!(KB_GT, Key::char('>'), "indent");
bind!(KB_LT, Key::char('<'), "deindent");

bind!(KB_BACKTICK, Key::char('`'), "to_lowercase");
bind!(KB_TILDE, Key::char('~'), "to_uppercase");
bind!(KB_BACKTICK_ALT, Key::alt('`'), "swap_case");

bind!(KB_J_ALT, Key::alt('j'), "join_lines");

bind!(KB_CTRL_U, Key::ctrl('u'), "scroll_half_page_up");
bind!(KB_CTRL_D, Key::ctrl('d'), "scroll_half_page_down");
bind!(KB_CTRL_B, Key::ctrl('b'), "scroll_page_up");
bind!(KB_CTRL_F, Key::ctrl('f'), "scroll_page_down");

bind!(KB_V, Key::char('v'), "view_mode");
bind!(KB_COLON, Key::char(':'), "command_mode");

bind!(KB_F, Key::char('f'), "find_char");
bind!(KB_T, Key::char('t'), "find_char_to");
bind!(KB_F_ALT, Key::alt('f'), "find_char_reverse");
bind!(KB_T_ALT, Key::alt('t'), "find_char_to_reverse");

bind!(KB_R, Key::char('r'), "replace_char");

bind!(KB_ALT_I, Key::alt('i'), "select_object_inner");
bind!(KB_ALT_A, Key::alt('a'), "select_object_around");
bind!(KB_BRACKET_OPEN, Key::char('['), "select_object_to_start");
bind!(KB_BRACKET_CLOSE, Key::char(']'), "select_object_to_end");
bind!(KB_BRACE_OPEN, Key::char('{'), "select_object_to_start");
bind!(KB_BRACE_CLOSE, Key::char('}'), "select_object_to_end");

bind!(KB_SLASH, Key::char('/'), "search_forward");
bind!(KB_QUESTION, Key::char('?'), "search_backward");
bind!(KB_N, Key::char('n'), "search_next");
bind!(KB_N_UPPER, Key::char('N'), "search_next_add");
bind!(KB_N_ALT, Key::alt('n'), "search_prev");
bind!(KB_N_ALT_UPPER, Key::alt('N'), "search_prev_add");
bind!(KB_STAR, Key::char('*'), "use_selection_as_search");

bind!(KB_S, Key::char('s'), "select_regex");
bind!(KB_S_UPPER, Key::char('S'), "split_regex");
bind!(KB_S_ALT, Key::alt('s'), "split_lines");
bind!(KB_K_ALT, Key::alt('k'), "keep_matching");
bind!(KB_K_ALT_UPPER, Key::alt('K'), "keep_not_matching");

bind!(KB_CTRL_I, Key::ctrl('i'), "jump_forward");
bind!(KB_CTRL_O, Key::ctrl('o'), "jump_backward");
bind!(KB_CTRL_S, Key::ctrl('s'), "save_jump");

bind!(KB_Q_UPPER, Key::char('Q'), "record_macro");
bind!(KB_Q, Key::char('q'), "play_macro");

bind!(KB_Z_UPPER, Key::char('Z'), "save_selections");
bind!(KB_Z, Key::char('z'), "restore_selections");

bind!(KB_CTRL_L, Key::ctrl('l'), "force_redraw");

bind!(KB_DOT, Key::char('.'), "repeat_last_insert");
bind!(KB_DOT_ALT, Key::alt('.'), "repeat_last_object");

bind!(KB_PIPE, Key::char('|'), "pipe_replace");
bind!(KB_PIPE_ALT, Key::alt('|'), "pipe_ignore");
bind!(KB_BANG, Key::char('!'), "insert_output");
bind!(KB_BANG_ALT, Key::alt('!'), "append_output");

bind!(KB_PLUS_DUP, Key::char('+'), "duplicate_selections_down");
bind!(KB_C_DUP, Key::char('C'), "duplicate_selections_down");
bind!(KB_C_ALT_DUP, Key::alt('C'), "duplicate_selections_up");
bind!(KB_PLUS_ALT_MERGE, Key::alt('+'), "merge_selections");
bind!(KB_AMP_ALIGN, Key::char('&'), "align");
bind!(KB_AMP_ALT_COPY_INDENT, Key::alt('&'), "copy_indent");
bind!(KB_AT_TABS, Key::char('@'), "tabs_to_spaces");
bind!(KB_AT_ALT_SPACES, Key::alt('@'), "spaces_to_tabs");
bind!(KB_UNDERSCORE_TRIM, Key::char('_'), "trim_selections");
