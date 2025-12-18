//! Insert mode keybindings.

use linkme::distributed_slice;

use crate::ext::keybindings::{BindingMode, KEYBINDINGS_INSERT, KeyBindingDef};
use crate::key::{Key, SpecialKey};

const DEFAULT_PRIORITY: i16 = 100;

macro_rules! bind {
	($name:ident, $key:expr, $action:expr) => {
		#[distributed_slice(KEYBINDINGS_INSERT)]
		static $name: KeyBindingDef = KeyBindingDef {
			mode: BindingMode::Insert,
			key: $key,
			action: $action,
			priority: DEFAULT_PRIORITY,
		};
	};
}

bind!(KB_INS_LEFT, Key::special(SpecialKey::Left), "move_left");
bind!(KB_INS_RIGHT, Key::special(SpecialKey::Right), "move_right");
bind!(KB_INS_UP, Key::special(SpecialKey::Up), "move_up_visual");
bind!(
	KB_INS_DOWN,
	Key::special(SpecialKey::Down),
	"move_down_visual"
);
bind!(
	KB_INS_HOME,
	Key::special(SpecialKey::Home),
	"move_line_start"
);
bind!(KB_INS_END, Key::special(SpecialKey::End), "move_line_end");

bind!(
	KB_INS_CTRL_LEFT,
	Key::special(SpecialKey::Left).with_ctrl(),
	"prev_word_start"
);
bind!(
	KB_INS_CTRL_RIGHT,
	Key::special(SpecialKey::Right).with_ctrl(),
	"next_word_start"
);

bind!(
	KB_INS_CTRL_HOME,
	Key::special(SpecialKey::Home).with_ctrl(),
	"document_start"
);
bind!(
	KB_INS_CTRL_END,
	Key::special(SpecialKey::End).with_ctrl(),
	"document_end"
);

bind!(
	KB_INS_PAGE_UP,
	Key::special(SpecialKey::PageUp),
	"scroll_page_up"
);
bind!(
	KB_INS_PAGE_DOWN,
	Key::special(SpecialKey::PageDown),
	"scroll_page_down"
);

bind!(KB_INS_CTRL_W, Key::ctrl('w'), "delete_word_back");
bind!(KB_INS_ALT_D, Key::alt('d'), "delete_word_forward");
bind!(KB_INS_CTRL_K, Key::ctrl('k'), "delete_to_line_end");
bind!(KB_INS_CTRL_U, Key::ctrl('u'), "delete_to_line_start");
