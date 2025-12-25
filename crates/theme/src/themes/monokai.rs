use linkme::distributed_slice;
use ratatui::style::{Color, Modifier};

use crate::{
	NotificationColors, PopupColors, StatusColors, SyntaxStyle, SyntaxStyles, THEMES, Theme,
	ThemeColors, UiColors,
};

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
		notification: NotificationColors::INHERITED,
		syntax: monokai_syntax(),
	},
	priority: 0,
	source: tome_manifest::RegistrySource::Builtin,
};

// Monokai palette
const RED: Color = Color::Rgb(249, 38, 114); // #F92672
const GREEN: Color = Color::Rgb(166, 226, 46); // #A6E22E
const YELLOW: Color = Color::Rgb(230, 219, 116); // #E6DB74
const BLUE: Color = Color::Rgb(102, 217, 239); // #66D9EF
const PURPLE: Color = Color::Rgb(174, 129, 255); // #AE81FF
const ORANGE: Color = Color::Rgb(253, 151, 31); // #FD971F
const GRAY: Color = Color::Rgb(117, 113, 94); // #75715E

const fn monokai_syntax() -> SyntaxStyles {
	let mut s = SyntaxStyles::minimal();

	// Comments - gray
	s.comment = SyntaxStyle::fg(GRAY);
	s.comment_line = s.comment;
	s.comment_block = s.comment;
	s.comment_block_documentation = s.comment;

	// Keywords - red
	s.keyword = SyntaxStyle::fg(RED);
	s.keyword_control = SyntaxStyle::fg(RED);
	s.keyword_control_conditional = SyntaxStyle::fg(RED);
	s.keyword_control_repeat = SyntaxStyle::fg(RED);
	s.keyword_control_import = SyntaxStyle::fg(RED);
	s.keyword_control_return = SyntaxStyle::fg(RED);
	s.keyword_control_exception = SyntaxStyle::fg(RED);
	s.keyword_operator = SyntaxStyle::fg(RED);
	s.keyword_directive = SyntaxStyle::fg(RED);
	s.keyword_function = SyntaxStyle::fg(BLUE);
	s.keyword_storage = SyntaxStyle::fg(RED);
	s.keyword_storage_type = SyntaxStyle::fg(BLUE);
	s.keyword_storage_modifier = SyntaxStyle::fg(RED);

	// Functions - green
	s.function = SyntaxStyle::fg(GREEN);
	s.function_builtin = SyntaxStyle::fg(GREEN);
	s.function_method = SyntaxStyle::fg(GREEN);
	s.function_macro = SyntaxStyle::fg(GREEN);
	s.function_special = SyntaxStyle::fg(GREEN);

	// Types - blue italic
	s.r#type = SyntaxStyle::fg_mod(BLUE, Modifier::ITALIC);
	s.type_builtin = SyntaxStyle::fg_mod(BLUE, Modifier::ITALIC);
	s.type_parameter = SyntaxStyle::fg_mod(BLUE, Modifier::ITALIC);
	s.type_enum_variant = SyntaxStyle::fg(BLUE);

	// Strings - yellow
	s.string = SyntaxStyle::fg(YELLOW);
	s.string_regexp = SyntaxStyle::fg(YELLOW);
	s.string_special = SyntaxStyle::fg(ORANGE);
	s.string_special_path = SyntaxStyle::fg(YELLOW);
	s.string_special_url = SyntaxStyle::fg_mod(YELLOW, Modifier::UNDERLINED);
	s.string_special_symbol = SyntaxStyle::fg(PURPLE);

	// Constants - purple
	s.constant = SyntaxStyle::fg(PURPLE);
	s.constant_builtin = SyntaxStyle::fg(PURPLE);
	s.constant_builtin_boolean = SyntaxStyle::fg(PURPLE);
	s.constant_character = SyntaxStyle::fg(PURPLE);
	s.constant_character_escape = SyntaxStyle::fg(PURPLE);
	s.constant_numeric = SyntaxStyle::fg(PURPLE);
	s.constant_numeric_integer = SyntaxStyle::fg(PURPLE);
	s.constant_numeric_float = SyntaxStyle::fg(PURPLE);

	// Variables - orange for parameters
	s.variable = SyntaxStyle::NONE;
	s.variable_builtin = SyntaxStyle::fg(ORANGE);
	s.variable_parameter = SyntaxStyle::fg_mod(ORANGE, Modifier::ITALIC);
	s.variable_other = SyntaxStyle::NONE;
	s.variable_other_member = SyntaxStyle::NONE;

	// Operators and punctuation
	s.operator = SyntaxStyle::fg(RED);
	s.punctuation = SyntaxStyle::NONE;
	s.punctuation_bracket = SyntaxStyle::NONE;
	s.punctuation_delimiter = SyntaxStyle::NONE;
	s.punctuation_special = SyntaxStyle::fg(RED);

	// Other
	s.attribute = SyntaxStyle::fg(GREEN);
	s.tag = SyntaxStyle::fg(RED);
	s.namespace = SyntaxStyle::NONE;
	s.constructor = SyntaxStyle::fg(GREEN);
	s.label = SyntaxStyle::NONE;
	s.special = SyntaxStyle::fg(BLUE);

	// Markup
	s.markup_heading = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_heading_1 = SyntaxStyle::fg_mod(RED, Modifier::BOLD);
	s.markup_heading_2 = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_heading_3 = SyntaxStyle::fg_mod(YELLOW, Modifier::BOLD);
	s.markup_bold = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_italic = SyntaxStyle::fg_mod(YELLOW, Modifier::ITALIC);
	s.markup_strikethrough = SyntaxStyle::fg_mod(GRAY, Modifier::CROSSED_OUT);
	s.markup_link = SyntaxStyle::fg(BLUE);
	s.markup_link_url = SyntaxStyle::fg_mod(BLUE, Modifier::UNDERLINED);
	s.markup_link_text = SyntaxStyle::fg(PURPLE);
	s.markup_quote = SyntaxStyle::fg_mod(GRAY, Modifier::ITALIC);
	s.markup_raw = SyntaxStyle::fg(GREEN);
	s.markup_raw_inline = SyntaxStyle::fg(GREEN);
	s.markup_raw_block = SyntaxStyle::fg(GREEN);
	s.markup_list = SyntaxStyle::fg(RED);

	// Diff
	s.diff_plus = SyntaxStyle::fg(GREEN);
	s.diff_minus = SyntaxStyle::fg(RED);
	s.diff_delta = SyntaxStyle::fg(ORANGE);

	s
}
