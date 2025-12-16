//! Insert mode keybindings.

use linkme::distributed_slice;

use crate::ext::keybindings::{BindingMode, KeyBindingDef, KEYBINDINGS};
use crate::key::{Key, SpecialKey};

const DEFAULT_PRIORITY: i16 = 100;

macro_rules! bind {
    ($name:ident, $key:expr, $action:expr) => {
        #[distributed_slice(KEYBINDINGS)]
        static $name: KeyBindingDef = KeyBindingDef {
            mode: BindingMode::Insert,
            key: $key,
            action: $action,
            priority: DEFAULT_PRIORITY,
        };
    };
}

// Navigation in insert mode
bind!(KB_INS_LEFT, Key::special(SpecialKey::Left), "move_left");
bind!(KB_INS_RIGHT, Key::special(SpecialKey::Right), "move_right");
bind!(KB_INS_UP, Key::special(SpecialKey::Up), "move_up_visual");
bind!(KB_INS_DOWN, Key::special(SpecialKey::Down), "move_down_visual");
bind!(KB_INS_HOME, Key::special(SpecialKey::Home), "move_line_start");
bind!(KB_INS_END, Key::special(SpecialKey::End), "move_line_end");

// Word navigation  
bind!(KB_INS_CTRL_LEFT, Key::special(SpecialKey::Left).with_ctrl(), "prev_word_start");
bind!(KB_INS_CTRL_RIGHT, Key::special(SpecialKey::Right).with_ctrl(), "next_word_start");

// Document navigation
bind!(KB_INS_CTRL_HOME, Key::special(SpecialKey::Home).with_ctrl(), "document_start");
bind!(KB_INS_CTRL_END, Key::special(SpecialKey::End).with_ctrl(), "document_end");

// Page navigation
bind!(KB_INS_PAGE_UP, Key::special(SpecialKey::PageUp), "scroll_page_up");
bind!(KB_INS_PAGE_DOWN, Key::special(SpecialKey::PageDown), "scroll_page_down");

// Delete word backward (Ctrl+W)
bind!(KB_INS_CTRL_W, Key::ctrl('w'), "delete_word_back");

// Delete word forward (Alt+D)
bind!(KB_INS_ALT_D, Key::alt('d'), "delete_word_forward");

// Delete to end of line (Ctrl+K)
bind!(KB_INS_CTRL_K, Key::ctrl('k'), "delete_to_line_end");

// Delete to start of line (Ctrl+U)
bind!(KB_INS_CTRL_U, Key::ctrl('u'), "delete_to_line_start");
