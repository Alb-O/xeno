use linkme::distributed_slice;
use ratatui::style::Color;

use crate::theme::{PopupColors, StatusColors, THEMES, Theme, ThemeColors, UiColors};

#[distributed_slice(THEMES)]
pub static SOLARIZED_DARK: Theme = Theme {
	name: "solarized_dark",
	aliases: &["solarized"],
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Rgb(0, 43, 54),                   // base03
			fg: Color::Rgb(131, 148, 150),               // base0
			gutter_fg: Color::Rgb(88, 110, 117),         // base01
			cursor_bg: Color::Rgb(147, 161, 161),        // base1
			cursor_fg: Color::Rgb(0, 43, 54),            // base03
			selection_bg: Color::Rgb(7, 54, 66),         // base02
			selection_fg: Color::Rgb(147, 161, 161),     // base1
			message_fg: Color::Rgb(181, 137, 0),         // yellow
			command_input_fg: Color::Rgb(131, 148, 150), // base0
		},
		status: StatusColors {
			normal_bg: Color::Rgb(38, 139, 210), // blue
			normal_fg: Color::Rgb(0, 43, 54),    // base03
			insert_bg: Color::Rgb(133, 153, 0),  // green
			insert_fg: Color::Rgb(0, 43, 54),    // base03
			goto_bg: Color::Rgb(211, 54, 130),   // magenta
			goto_fg: Color::Rgb(253, 246, 227),  // base3
			view_bg: Color::Rgb(42, 161, 152),   // cyan
			view_fg: Color::Rgb(0, 43, 54),      // base03
			command_bg: Color::Rgb(181, 137, 0), // yellow
			command_fg: Color::Rgb(0, 43, 54),   // base03

			dim_fg: Color::Rgb(88, 110, 117),    // base01
			warning_fg: Color::Rgb(203, 75, 22), // orange
			error_fg: Color::Rgb(220, 50, 47),   // red
			success_fg: Color::Rgb(133, 153, 0), // green
		},
		popup: PopupColors {
			bg: Color::Rgb(7, 54, 66),        // base02
			fg: Color::Rgb(131, 148, 150),    // base0
			border: Color::Rgb(88, 110, 117), // base01
			title: Color::Rgb(181, 137, 0),   // yellow
		},
	},
};
