use linkme::distributed_slice;
use ratatui::style::Color;

use crate::theme::{PopupColors, StatusColors, THEMES, Theme, ThemeColors, UiColors};

#[distributed_slice(THEMES)]
pub static MONOKAI: Theme = Theme {
	id: "monokai",
	name: "monokai",
	aliases: &["monokai_extended"],
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Rgb(39, 40, 34),                  // #272822
			fg: Color::Rgb(248, 248, 242),               // #F8F8F2
			gutter_fg: Color::Rgb(117, 113, 94),         // #75715E
			cursor_bg: Color::Rgb(248, 248, 240),        // #F8F8F0
			cursor_fg: Color::Rgb(39, 40, 34),           // #272822
			selection_bg: Color::Rgb(73, 72, 62),        // #49483E
			selection_fg: Color::Rgb(248, 248, 242),     // #F8F8F2
			message_fg: Color::Rgb(230, 219, 116),       // #E6DB74 (Yellow)
			command_input_fg: Color::Rgb(248, 248, 242), // #F8F8F2
		},
		status: StatusColors {
			normal_bg: Color::Rgb(102, 217, 239),  // #66D9EF (Blue)
			normal_fg: Color::Rgb(39, 40, 34),     // #272822
			insert_bg: Color::Rgb(166, 226, 46),   // #A6E22E (Green)
			insert_fg: Color::Rgb(39, 40, 34),     // #272822
			goto_bg: Color::Rgb(174, 129, 255),    // #AE81FF (Purple)
			goto_fg: Color::Rgb(248, 248, 242),    // #F8F8F2
			view_bg: Color::Rgb(253, 151, 31),     // #FD971F (Orange)
			view_fg: Color::Rgb(39, 40, 34),       // #272822
			command_bg: Color::Rgb(230, 219, 116), // #E6DB74 (Yellow)
			command_fg: Color::Rgb(39, 40, 34),    // #272822

			dim_fg: Color::Rgb(117, 113, 94),     // #75715E
			warning_fg: Color::Rgb(253, 151, 31), // #FD971F (Orange)
			error_fg: Color::Rgb(249, 38, 114),   // #F92672 (Red)
			success_fg: Color::Rgb(166, 226, 46), // #A6E22E (Green)
		},
		popup: PopupColors {
			bg: Color::Rgb(30, 31, 28),       // #1E1F1C (Darker)
			fg: Color::Rgb(248, 248, 242),    // #F8F8F2
			border: Color::Rgb(117, 113, 94), // #75715E
			title: Color::Rgb(230, 219, 116), // #E6DB74
		},
	},
	priority: 0,
	source: tome_core::ext::ExtensionSource::Builtin,
};
