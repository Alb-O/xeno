use linkme::distributed_slice;

use crate::{
	Color, Modifier, NotificationColors, PopupColors, StatusColors, SyntaxStyle, SyntaxStyles,
	THEMES, Theme, ThemeColors, ThemeVariant, UiColors,
};

#[distributed_slice(THEMES)]
pub static GRUVBOX: Theme = Theme {
	id: "gruvbox",
	name: "gruvbox",
	aliases: &["gruvbox_dark"],
	variant: ThemeVariant::Dark,
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Rgb(40, 40, 40),                  // #282828
			fg: Color::Rgb(235, 219, 178),               // #EBDBB2
			gutter_fg: Color::Rgb(146, 131, 116),        // #928374
			cursor_bg: Color::Rgb(235, 219, 178),        // #EBDBB2
			cursor_fg: Color::Rgb(40, 40, 40),           // #282828
			cursorline_bg: Color::Rgb(50, 48, 47),       // #32302F (slightly lighter than bg)
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
		// Inherit notification colors from popup/status (no overrides)
		notification: NotificationColors::INHERITED,
		syntax: gruvbox_syntax(),
	},
	priority: 0,
	source: evildoer_manifest::RegistrySource::Builtin,
};

// Gruvbox palette
const RED: Color = Color::Rgb(251, 73, 52); // #FB4934
const GREEN: Color = Color::Rgb(184, 187, 38); // #B8BB26
const YELLOW: Color = Color::Rgb(250, 189, 47); // #FABD2F
const BLUE: Color = Color::Rgb(131, 165, 152); // #83A598
const PURPLE: Color = Color::Rgb(211, 134, 155); // #D3869B
const AQUA: Color = Color::Rgb(142, 192, 124); // #8EC07C
const ORANGE: Color = Color::Rgb(254, 128, 25); // #FE8019
const GRAY: Color = Color::Rgb(146, 131, 116); // #928374

const fn gruvbox_syntax() -> SyntaxStyles {
	let mut s = SyntaxStyles::minimal();

	// Comments - gray italic
	s.comment = SyntaxStyle::fg_mod(GRAY, Modifier::ITALIC);
	s.comment_line = s.comment;
	s.comment_block = s.comment;
	s.comment_block_documentation = SyntaxStyle::fg_mod(GRAY, Modifier::ITALIC);

	// Keywords - red
	s.keyword = SyntaxStyle::fg(RED);
	s.keyword_control = SyntaxStyle::fg(RED);
	s.keyword_control_conditional = SyntaxStyle::fg(RED);
	s.keyword_control_repeat = SyntaxStyle::fg(RED);
	s.keyword_control_import = SyntaxStyle::fg(RED);
	s.keyword_control_return = SyntaxStyle::fg(RED);
	s.keyword_control_exception = SyntaxStyle::fg(RED);
	s.keyword_operator = SyntaxStyle::fg(RED);
	s.keyword_function = SyntaxStyle::fg(AQUA);
	s.keyword_storage = SyntaxStyle::fg(ORANGE);
	s.keyword_storage_type = SyntaxStyle::fg(YELLOW);
	s.keyword_storage_modifier = SyntaxStyle::fg(ORANGE);

	// Functions - green
	s.function = SyntaxStyle::fg(GREEN);
	s.function_builtin = SyntaxStyle::fg(GREEN);
	s.function_method = SyntaxStyle::fg(GREEN);
	s.function_macro = SyntaxStyle::fg(AQUA);
	s.function_special = SyntaxStyle::fg(GREEN);

	// Types - yellow
	s.r#type = SyntaxStyle::fg(YELLOW);
	s.type_builtin = SyntaxStyle::fg(YELLOW);
	s.type_parameter = SyntaxStyle::fg(YELLOW);
	s.type_enum_variant = SyntaxStyle::fg(AQUA);

	// Strings - green
	s.string = SyntaxStyle::fg(GREEN);
	s.string_regexp = SyntaxStyle::fg(GREEN);
	s.string_special = SyntaxStyle::fg(ORANGE);
	s.string_special_path = SyntaxStyle::fg(AQUA);
	s.string_special_url = SyntaxStyle::fg_mod(AQUA, Modifier::UNDERLINED);
	s.string_special_symbol = SyntaxStyle::fg(PURPLE);

	// Constants - purple
	s.constant = SyntaxStyle::fg(PURPLE);
	s.constant_builtin = SyntaxStyle::fg(PURPLE);
	s.constant_builtin_boolean = SyntaxStyle::fg(PURPLE);
	s.constant_character = SyntaxStyle::fg(PURPLE);
	s.constant_character_escape = SyntaxStyle::fg(ORANGE);
	s.constant_numeric = SyntaxStyle::fg(PURPLE);
	s.constant_numeric_integer = SyntaxStyle::fg(PURPLE);
	s.constant_numeric_float = SyntaxStyle::fg(PURPLE);

	// Variables - blue
	s.variable = SyntaxStyle::fg(BLUE);
	s.variable_builtin = SyntaxStyle::fg(ORANGE);
	s.variable_parameter = SyntaxStyle::fg(BLUE);
	s.variable_other = SyntaxStyle::fg(BLUE);
	s.variable_other_member = SyntaxStyle::fg(BLUE);

	// Operators and punctuation
	s.operator = SyntaxStyle::fg(AQUA);
	s.punctuation = SyntaxStyle::NONE;
	s.punctuation_bracket = SyntaxStyle::NONE;
	s.punctuation_delimiter = SyntaxStyle::NONE;
	s.punctuation_special = SyntaxStyle::fg(ORANGE);

	// Other
	s.attribute = SyntaxStyle::fg(AQUA);
	s.tag = SyntaxStyle::fg(AQUA);
	s.namespace = SyntaxStyle::fg(BLUE);
	s.constructor = SyntaxStyle::fg(YELLOW);
	s.label = SyntaxStyle::fg(AQUA);
	s.special = SyntaxStyle::fg(ORANGE);

	// Markup
	s.markup_heading = SyntaxStyle::fg_mod(YELLOW, Modifier::BOLD);
	s.markup_heading_1 = SyntaxStyle::fg_mod(RED, Modifier::BOLD);
	s.markup_heading_2 = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_heading_3 = SyntaxStyle::fg_mod(YELLOW, Modifier::BOLD);
	s.markup_bold = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_italic = SyntaxStyle::fg_mod(PURPLE, Modifier::ITALIC);
	s.markup_strikethrough = SyntaxStyle::fg_mod(GRAY, Modifier::CROSSED_OUT);
	s.markup_link = SyntaxStyle::fg(AQUA);
	s.markup_link_url = SyntaxStyle::fg_mod(AQUA, Modifier::UNDERLINED);
	s.markup_link_text = SyntaxStyle::fg(PURPLE);
	s.markup_quote = SyntaxStyle::fg_mod(GRAY, Modifier::ITALIC);
	s.markup_raw = SyntaxStyle::fg(GREEN);
	s.markup_raw_inline = SyntaxStyle::fg(GREEN);
	s.markup_raw_block = SyntaxStyle::fg(GREEN);
	s.markup_list = SyntaxStyle::fg(RED);

	// Diff
	s.diff_plus = SyntaxStyle::fg(GREEN);
	s.diff_minus = SyntaxStyle::fg(RED);
	s.diff_delta = SyntaxStyle::fg(YELLOW);

	s
}
