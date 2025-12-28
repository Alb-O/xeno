use linkme::distributed_slice;

use crate::{
	Color, Modifier, NotificationColors, PopupColors, StatusColors, SyntaxStyle, SyntaxStyles,
	THEMES, Theme, ThemeColors, ThemeVariant, UiColors,
};

#[distributed_slice(THEMES)]
pub static ONE_DARK: Theme = Theme {
	id: "one_dark",
	name: "one_dark",
	aliases: &["atom_one_dark", "one"],
	variant: ThemeVariant::Dark,
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Rgb(40, 44, 52),                  // #282C34
			fg: Color::Rgb(171, 178, 191),               // #ABB2BF
			gutter_fg: Color::Rgb(92, 99, 112),          // #5C6370
			cursor_bg: Color::Rgb(82, 139, 255),         // #528BFF (Blue-ish)
			cursor_fg: Color::Rgb(40, 44, 52),           // #282C34
			cursorline_bg: Color::Rgb(44, 49, 58),       // #2C313A (slightly lighter than bg)
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
		notification: NotificationColors::INHERITED,
		syntax: one_dark_syntax(),
	},
	priority: 0,
	source: evildoer_manifest::RegistrySource::Builtin,
};

// One Dark palette
const RED: Color = Color::Rgb(224, 108, 117); // #E06C75
const GREEN: Color = Color::Rgb(152, 195, 121); // #98C379
const YELLOW: Color = Color::Rgb(229, 192, 123); // #E5C07B
const BLUE: Color = Color::Rgb(97, 175, 239); // #61AFEF
const PURPLE: Color = Color::Rgb(198, 120, 221); // #C678DD
const CYAN: Color = Color::Rgb(86, 182, 194); // #56B6C2
const ORANGE: Color = Color::Rgb(209, 154, 102); // #D19A66
const GRAY: Color = Color::Rgb(92, 99, 112); // #5C6370

const fn one_dark_syntax() -> SyntaxStyles {
	let mut s = SyntaxStyles::minimal();

	// Comments - gray italic
	s.comment = SyntaxStyle::fg_mod(GRAY, Modifier::ITALIC);
	s.comment_line = s.comment;
	s.comment_block = s.comment;
	s.comment_block_documentation = s.comment;

	// Keywords - red/purple
	s.keyword = SyntaxStyle::fg(RED);
	s.keyword_control = SyntaxStyle::fg(PURPLE);
	s.keyword_control_conditional = SyntaxStyle::fg(PURPLE);
	s.keyword_control_repeat = SyntaxStyle::fg(PURPLE);
	s.keyword_control_import = SyntaxStyle::fg(RED);
	s.keyword_control_return = SyntaxStyle::fg(PURPLE);
	s.keyword_control_exception = SyntaxStyle::fg(PURPLE);
	s.keyword_operator = SyntaxStyle::fg(PURPLE);
	s.keyword_directive = SyntaxStyle::fg(PURPLE);
	s.keyword_function = SyntaxStyle::fg(CYAN);
	s.keyword_storage = SyntaxStyle::fg(PURPLE);
	s.keyword_storage_type = SyntaxStyle::fg(PURPLE);
	s.keyword_storage_modifier = SyntaxStyle::fg(PURPLE);

	// Functions - blue
	s.function = SyntaxStyle::fg(BLUE);
	s.function_builtin = SyntaxStyle::fg(BLUE);
	s.function_method = SyntaxStyle::fg(BLUE);
	s.function_macro = SyntaxStyle::fg(PURPLE);
	s.function_special = SyntaxStyle::fg(BLUE);

	// Types - yellow
	s.r#type = SyntaxStyle::fg(YELLOW);
	s.type_builtin = SyntaxStyle::fg(YELLOW);
	s.type_parameter = SyntaxStyle::fg(YELLOW);
	s.type_enum_variant = SyntaxStyle::fg(CYAN);

	// Strings - green
	s.string = SyntaxStyle::fg(GREEN);
	s.string_regexp = SyntaxStyle::fg(GREEN);
	s.string_special = SyntaxStyle::fg(ORANGE);
	s.string_special_path = SyntaxStyle::fg(CYAN);
	s.string_special_url = SyntaxStyle::fg_mod(CYAN, Modifier::UNDERLINED);
	s.string_special_symbol = SyntaxStyle::fg(PURPLE);

	// Constants - orange/purple
	s.constant = SyntaxStyle::fg(CYAN);
	s.constant_builtin = SyntaxStyle::fg(ORANGE);
	s.constant_builtin_boolean = SyntaxStyle::fg(ORANGE);
	s.constant_character = SyntaxStyle::fg(ORANGE);
	s.constant_character_escape = SyntaxStyle::fg(ORANGE);
	s.constant_numeric = SyntaxStyle::fg(ORANGE);
	s.constant_numeric_integer = SyntaxStyle::fg(ORANGE);
	s.constant_numeric_float = SyntaxStyle::fg(ORANGE);

	// Variables
	s.variable = SyntaxStyle::NONE; // Use default text color
	s.variable_builtin = SyntaxStyle::fg(BLUE);
	s.variable_parameter = SyntaxStyle::fg(RED);
	s.variable_other = SyntaxStyle::NONE;
	s.variable_other_member = SyntaxStyle::fg(RED);

	// Operators and punctuation
	s.operator = SyntaxStyle::fg(PURPLE);
	s.punctuation = SyntaxStyle::NONE;
	s.punctuation_bracket = SyntaxStyle::NONE;
	s.punctuation_delimiter = SyntaxStyle::NONE;
	s.punctuation_special = SyntaxStyle::fg(CYAN);

	// Other
	s.attribute = SyntaxStyle::fg(YELLOW);
	s.tag = SyntaxStyle::fg(RED);
	s.namespace = SyntaxStyle::fg(BLUE);
	s.constructor = SyntaxStyle::fg(BLUE);
	s.label = SyntaxStyle::fg(PURPLE);
	s.special = SyntaxStyle::fg(BLUE);

	// Markup
	s.markup_heading = SyntaxStyle::fg_mod(RED, Modifier::BOLD);
	s.markup_heading_1 = SyntaxStyle::fg_mod(RED, Modifier::BOLD);
	s.markup_heading_2 = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_heading_3 = SyntaxStyle::fg_mod(YELLOW, Modifier::BOLD);
	s.markup_bold = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_italic = SyntaxStyle::fg_mod(PURPLE, Modifier::ITALIC);
	s.markup_strikethrough = SyntaxStyle::fg_mod(GRAY, Modifier::CROSSED_OUT);
	s.markup_link = SyntaxStyle::fg(PURPLE);
	s.markup_link_url = SyntaxStyle::fg_mod(CYAN, Modifier::UNDERLINED);
	s.markup_link_text = SyntaxStyle::fg(PURPLE);
	s.markup_quote = SyntaxStyle::fg_mod(YELLOW, Modifier::ITALIC);
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
