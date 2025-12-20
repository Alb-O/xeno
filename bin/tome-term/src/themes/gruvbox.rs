use linkme::distributed_slice;
use ratatui::style::Color;

use crate::theme::{PopupColors, StatusColors, THEMES, Theme, ThemeColors, UiColors};

#[distributed_slice(THEMES)]
pub static GRUVBOX: Theme = Theme {
	id: "gruvbox",
	name: "gruvbox",
	aliases: &["gruvbox_dark"],
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Rgb(40, 40, 40),                  // #282828
			fg: Color::Rgb(235, 219, 178),               // #EBDBB2
			gutter_fg: Color::Rgb(146, 131, 116),        // #928374
			cursor_bg: Color::Rgb(235, 219, 178),        // #EBDBB2
			cursor_fg: Color::Rgb(40, 40, 40),           // #282828
			selection_bg: Color::Rgb(80, 73, 69),        // #504945
			selection_fg: Color::Rgb(235, 219, 178),     // #EBDBB2
			message_fg: Color::Rgb(250, 189, 47),        // #FABD2F (Yellow)
			command_input_fg: Color::Rgb(235, 219, 178), // #EBDBB2
		},
		status: StatusColors {
			normal_bg: Color::Rgb(131, 165, 152), // #83A598 (Blue)
			normal_fg: Color::Rgb(40, 40, 40),    // #282828
			insert_bg: Color::Rgb(184, 187, 38),  // #B8BB26 (Green)
			insert_fg: Color::Rgb(40, 40, 40),    // #282828
			goto_bg: Color::Rgb(211, 134, 155),   // #D3869B (Purple)
			goto_fg: Color::Rgb(40, 40, 40),      // #282828
			view_bg: Color::Rgb(254, 128, 25),    // #FE8019 (Orange)
			view_fg: Color::Rgb(40, 40, 40),      // #282828
			command_bg: Color::Rgb(250, 189, 47), // #FABD2F (Yellow)
			command_fg: Color::Rgb(40, 40, 40),   // #282828

			dim_fg: Color::Rgb(146, 131, 116),    // #928374
			warning_fg: Color::Rgb(250, 189, 47), // #FABD2F
			error_fg: Color::Rgb(251, 73, 52),    // #FB4934 (Red)
			success_fg: Color::Rgb(184, 187, 38), // #B8BB26
		},
		popup: PopupColors {
			bg: Color::Rgb(50, 48, 47),        // #32302F (Darker)
			fg: Color::Rgb(235, 219, 178),     // #EBDBB2
			border: Color::Rgb(146, 131, 116), // #928374
			title: Color::Rgb(184, 187, 38),   // #B8BB26
		},
	},
	priority: 0,
	source: tome_core::ext::ExtensionSource::Builtin,
};
