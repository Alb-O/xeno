use linkme::distributed_slice;
use ratatui::style::Color;

use crate::theme::{PopupColors, StatusColors, THEMES, Theme, ThemeColors, UiColors};

#[distributed_slice(THEMES)]
pub static ONE_DARK: Theme = Theme {
	name: "one_dark",
	aliases: &["atom_one_dark", "one"],
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Rgb(40, 44, 52),                  // #282C34
			fg: Color::Rgb(171, 178, 191),               // #ABB2BF
			gutter_fg: Color::Rgb(92, 99, 112),          // #5C6370
			cursor_bg: Color::Rgb(82, 139, 255),         // #528BFF (Blue-ish)
			cursor_fg: Color::Rgb(40, 44, 52),           // #282C34
			selection_bg: Color::Rgb(62, 68, 81),        // #3E4451
			selection_fg: Color::Rgb(171, 178, 191),     // #ABB2BF
			message_fg: Color::Rgb(229, 192, 123),       // #E5C07B (Yellow)
			command_input_fg: Color::Rgb(171, 178, 191), // #ABB2BF
		},
		status: StatusColors {
			normal_bg: Color::Rgb(97, 175, 239),   // #61AFEF (Blue)
			normal_fg: Color::Rgb(40, 44, 52),     // #282C34
			insert_bg: Color::Rgb(152, 195, 121),  // #98C379 (Green)
			insert_fg: Color::Rgb(40, 44, 52),     // #282C34
			goto_bg: Color::Rgb(198, 120, 221),    // #C678DD (Purple)
			goto_fg: Color::Rgb(40, 44, 52),       // #282C34
			view_bg: Color::Rgb(229, 192, 123),    // #E5C07B (Yellow/Orange)
			view_fg: Color::Rgb(40, 44, 52),       // #282C34
			command_bg: Color::Rgb(209, 154, 102), // #D19A66 (Orange)
			command_fg: Color::Rgb(40, 44, 52),    // #282C34

			dim_fg: Color::Rgb(92, 99, 112),       // #5C6370
			warning_fg: Color::Rgb(229, 192, 123), // #E5C07B
			error_fg: Color::Rgb(224, 108, 117),   // #E06C75 (Red)
			success_fg: Color::Rgb(152, 195, 121), // #98C379
		},
		popup: PopupColors {
			bg: Color::Rgb(33, 37, 43),      // #21252B (Darker)
			fg: Color::Rgb(171, 178, 191),   // #ABB2BF
			border: Color::Rgb(24, 26, 31),  // #181A1F
			title: Color::Rgb(97, 175, 239), // #61AFEF
		},
	},
};
