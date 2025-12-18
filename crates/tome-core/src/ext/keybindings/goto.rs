//! Default keybindings for goto mode.

use linkme::distributed_slice;

use crate::ext::keybindings::{BindingMode, KEYBINDINGS_GOTO, KeyBindingDef};
use crate::key::Key;

const DEFAULT_PRIORITY: i16 = 100;

macro_rules! bind {
	($name:ident, $key:expr, $action:expr) => {
		#[distributed_slice(KEYBINDINGS_GOTO)]
		static $name: KeyBindingDef = KeyBindingDef {
			mode: BindingMode::Goto,
			key: $key,
			action: $action,
			priority: DEFAULT_PRIORITY,
		};
	};
}

bind!(KB_GOTO_H, Key::char('h'), "move_line_start");
bind!(KB_GOTO_L, Key::char('l'), "move_line_end");
bind!(KB_GOTO_I, Key::char('i'), "move_first_nonblank");
bind!(KB_GOTO_G, Key::char('g'), "document_start");
bind!(KB_GOTO_K, Key::char('k'), "document_start");
bind!(KB_GOTO_J, Key::char('j'), "document_end");
bind!(KB_GOTO_E, Key::char('e'), "document_end");
