//! Default keybindings for window mode (Ctrl+w prefix).
//!
//! Window mode provides split and buffer navigation commands.

use linkme::distributed_slice;
use tome_base::key::Key;

use crate::keybindings::{BindingMode, KEYBINDINGS_WINDOW, KeyBindingDef};

const DEFAULT_PRIORITY: i16 = 100;

macro_rules! bind {
	($name:ident, $key:expr, $action:expr) => {
		#[distributed_slice(KEYBINDINGS_WINDOW)]
		static $name: KeyBindingDef = KeyBindingDef {
			mode: BindingMode::Window,
			key: $key,
			action: $action,
			priority: DEFAULT_PRIORITY,
		};
	};
}

// Split creation
bind!(KB_S, Key::char('s'), "split_horizontal");
bind!(KB_V, Key::char('v'), "split_vertical");

// Focus navigation between splits
bind!(KB_H, Key::char('h'), "focus_left");
bind!(KB_J, Key::char('j'), "focus_down");
bind!(KB_K, Key::char('k'), "focus_up");
bind!(KB_L, Key::char('l'), "focus_right");

// Buffer navigation (when no splits, cycle buffers)
bind!(KB_N, Key::char('n'), "buffer_next");
bind!(KB_P, Key::char('p'), "buffer_prev");

// Close current split/buffer
bind!(KB_Q, Key::char('q'), "close_buffer");
bind!(KB_C, Key::char('c'), "close_buffer");

// Only keep current split
bind!(KB_O, Key::char('o'), "close_other_buffers");
