//! Default keybindings for normal mode.
//!
//! Motion, scroll, selection, and mode keybindings are colocated with their
//! action definitions in `tome-stdlib` using `bound_action!`.

use linkme::distributed_slice;
use tome_base::key::Key;

use crate::keybindings::{BindingMode, KEYBINDINGS_NORMAL, KeyBindingDef};

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

bind!(KB_GT, Key::char('>'), "indent");
bind!(KB_LT, Key::char('<'), "deindent");

bind!(KB_BACKTICK, Key::char('`'), "to_lowercase");
bind!(KB_TILDE, Key::char('~'), "to_uppercase");
bind!(KB_BACKTICK_ALT, Key::alt('`'), "swap_case");

bind!(KB_J_ALT, Key::alt('j'), "join_lines");

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
