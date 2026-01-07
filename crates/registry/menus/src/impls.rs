use crate::{menu_group, menu_item};

/// Nerd Font icon codepoints (Font Awesome set, `nf-fa-*`).
mod icons {
	pub const FILE: u32 = 0xF15B;
	pub const FOLDER_OPEN: u32 = 0xF07C;
	pub const FLOPPY: u32 = 0xF0C7;
	pub const POWER_OFF: u32 = 0xF011;
	pub const UNDO: u32 = 0xF0E2;
	pub const REPEAT: u32 = 0xF01E;
	pub const SCISSORS: u32 = 0xF0C4;
	pub const COPY: u32 = 0xF0C5;
	pub const PASTE: u32 = 0xF0EA;
	pub const COLUMNS: u32 = 0xF0DB;
	pub const WINDOW: u32 = 0xF2D2;
	pub const TIMES: u32 = 0xF00D;
	pub const INFO: u32 = 0xF05A;
}

menu_group!(file, { label: "File", priority: 0 });
menu_item!(file_new, { group: "file", label: "New", command: "new", icon: icons::FILE, priority: 0 });
menu_item!(file_open, { group: "file", label: "Open…", command: "open", icon: icons::FOLDER_OPEN, priority: 10 });
menu_item!(file_save, { group: "file", label: "Save", command: "write", icon: icons::FLOPPY, priority: 20 });
menu_item!(file_save_as, { group: "file", label: "Save As…", command: "write-to", icon: icons::FLOPPY, priority: 30 });
menu_item!(file_quit, { group: "file", label: "Quit", command: "quit", icon: icons::POWER_OFF, priority: 100 });

menu_group!(edit, { label: "Edit", priority: 10 });
menu_item!(edit_undo, { group: "edit", label: "Undo", command: "undo", icon: icons::UNDO, priority: 0 });
menu_item!(edit_redo, { group: "edit", label: "Redo", command: "redo", icon: icons::REPEAT, priority: 10 });
menu_item!(edit_cut, { group: "edit", label: "Cut", command: "cut", icon: icons::SCISSORS, priority: 20 });
menu_item!(edit_copy, { group: "edit", label: "Copy", command: "copy", icon: icons::COPY, priority: 30 });
menu_item!(edit_paste, { group: "edit", label: "Paste", command: "paste", icon: icons::PASTE, priority: 40 });

menu_group!(view, { label: "View", priority: 20 });
menu_item!(view_split_horizontal, { group: "view", label: "Split Horizontal", command: "hsplit", icon: icons::COLUMNS, priority: 0 });
menu_item!(view_split_vertical, { group: "view", label: "Split Vertical", command: "vsplit", icon: icons::WINDOW, priority: 10 });
menu_item!(view_close_split, { group: "view", label: "Close Split", command: "close", icon: icons::TIMES, priority: 20 });

menu_group!(help, { label: "Help", priority: 100 });
menu_item!(help_about, { group: "help", label: "About", command: "about", icon: icons::INFO, priority: 0 });
