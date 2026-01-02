//! Standard library menu implementations.

use crate::{menu_group, menu_item};

menu_group!(file, {
	label: "File",
	priority: 0,
});

menu_item!(file_new, {
	group: "file",
	label: "New",
	command: "new",
	priority: 0,
});

menu_item!(file_open, {
	group: "file",
	label: "Open...",
	command: "open",
	priority: 10,
});

menu_item!(file_save, {
	group: "file",
	label: "Save",
	command: "write",
	priority: 20,
});

menu_item!(file_save_as, {
	group: "file",
	label: "Save As...",
	command: "write-to",
	priority: 30,
});

menu_item!(file_quit, {
	group: "file",
	label: "Quit",
	command: "quit",
	priority: 100,
});

menu_group!(edit, {
	label: "Edit",
	priority: 10,
});

menu_item!(edit_undo, {
	group: "edit",
	label: "Undo",
	command: "undo",
	priority: 0,
});

menu_item!(edit_redo, {
	group: "edit",
	label: "Redo",
	command: "redo",
	priority: 10,
});

menu_item!(edit_cut, {
	group: "edit",
	label: "Cut",
	command: "cut",
	priority: 20,
});

menu_item!(edit_copy, {
	group: "edit",
	label: "Copy",
	command: "copy",
	priority: 30,
});

menu_item!(edit_paste, {
	group: "edit",
	label: "Paste",
	command: "paste",
	priority: 40,
});

menu_group!(view, {
	label: "View",
	priority: 20,
});

menu_item!(view_split_horizontal, {
	group: "view",
	label: "Split Horizontal",
	command: "hsplit",
	priority: 0,
});

menu_item!(view_split_vertical, {
	group: "view",
	label: "Split Vertical",
	command: "vsplit",
	priority: 10,
});

menu_item!(view_close_split, {
	group: "view",
	label: "Close Split",
	command: "close",
	priority: 20,
});

menu_group!(help, {
	label: "Help",
	priority: 100,
});

menu_item!(help_about, {
	group: "help",
	label: "About",
	command: "about",
	priority: 0,
});
