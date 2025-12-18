//! Default keybindings for view mode.

use linkme::distributed_slice;

use crate::ext::keybindings::{BindingMode, KEYBINDINGS_VIEW, KeyBindingDef};
use crate::key::Key;

const DEFAULT_PRIORITY: i16 = 100;

macro_rules! bind {
	($name:ident, $key:expr, $action:expr) => {
		#[distributed_slice(KEYBINDINGS_VIEW)]
		static $name: KeyBindingDef = KeyBindingDef {
			mode: BindingMode::View,
			key: $key,
			action: $action,
			priority: DEFAULT_PRIORITY,
		};
	};
}

bind!(KB_VIEW_V, Key::char('v'), "center_cursor");
bind!(KB_VIEW_C, Key::char('c'), "center_cursor");
bind!(KB_VIEW_T, Key::char('t'), "cursor_to_top");
bind!(KB_VIEW_B, Key::char('b'), "cursor_to_bottom");
bind!(KB_VIEW_J, Key::char('j'), "scroll_down");
bind!(KB_VIEW_K, Key::char('k'), "scroll_up");
