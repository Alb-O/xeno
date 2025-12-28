use linkme::distributed_slice;

use crate::{
	Color, Modifier, NotificationColors, PopupColors, StatusColors, SyntaxStyle, SyntaxStyles,
	THEMES, Theme, ThemeColors, ThemeVariant, UiColors,
};

#[distributed_slice(THEMES)]
pub static SOLARIZED_DARK: Theme = Theme {
	id: "solarized_dark",
	name: "solarized_dark",
	aliases: &["solarized"],
	variant: ThemeVariant::Dark,
	colors: ThemeColors {
		ui: UiColors {
			bg: Color::Rgb(0, 43, 54),                   // base03
			fg: Color::Rgb(131, 148, 150),               // base0
			gutter_fg: Color::Rgb(88, 110, 117),         // base01
			cursor_bg: Color::Rgb(147, 161, 161),        // base1
			cursor_fg: Color::Rgb(0, 43, 54),            // base03
			cursorline_bg: Color::Rgb(7, 54, 66),        // base02 (one step lighter)
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
		notification: NotificationColors::INHERITED,
		syntax: solarized_syntax(),
	},
	priority: 0,
	source: evildoer_manifest::RegistrySource::Builtin,
};

// Solarized palette
const YELLOW: Color = Color::Rgb(181, 137, 0); // #B58900
const ORANGE: Color = Color::Rgb(203, 75, 22); // #CB4B16
const RED: Color = Color::Rgb(220, 50, 47); // #DC322F
const MAGENTA: Color = Color::Rgb(211, 54, 130); // #D33682
const VIOLET: Color = Color::Rgb(108, 113, 196); // #6C71C4
const BLUE: Color = Color::Rgb(38, 139, 210); // #268BD2
const CYAN: Color = Color::Rgb(42, 161, 152); // #2AA198
const GREEN: Color = Color::Rgb(133, 153, 0); // #859900
const BASE01: Color = Color::Rgb(88, 110, 117); // #586E75

const fn solarized_syntax() -> SyntaxStyles {
	let mut s = SyntaxStyles::minimal();

	// Comments - base01
	s.comment = SyntaxStyle::fg_mod(BASE01, Modifier::ITALIC);
	s.comment_line = s.comment;
	s.comment_block = s.comment;
	s.comment_block_documentation = s.comment;

	// Keywords - green
	s.keyword = SyntaxStyle::fg(GREEN);
	s.keyword_control = SyntaxStyle::fg(GREEN);
	s.keyword_control_conditional = SyntaxStyle::fg(GREEN);
	s.keyword_control_repeat = SyntaxStyle::fg(GREEN);
	s.keyword_control_import = SyntaxStyle::fg(ORANGE);
	s.keyword_control_return = SyntaxStyle::fg(GREEN);
	s.keyword_control_exception = SyntaxStyle::fg(ORANGE);
	s.keyword_operator = SyntaxStyle::fg(GREEN);
	s.keyword_directive = SyntaxStyle::fg(ORANGE);
	s.keyword_function = SyntaxStyle::fg(GREEN);
	s.keyword_storage = SyntaxStyle::fg(GREEN);
	s.keyword_storage_type = SyntaxStyle::fg(YELLOW);
	s.keyword_storage_modifier = SyntaxStyle::fg(GREEN);

	// Functions - blue
	s.function = SyntaxStyle::fg(BLUE);
	s.function_builtin = SyntaxStyle::fg(BLUE);
	s.function_method = SyntaxStyle::fg(BLUE);
	s.function_macro = SyntaxStyle::fg(ORANGE);
	s.function_special = SyntaxStyle::fg(BLUE);

	// Types - yellow
	s.r#type = SyntaxStyle::fg(YELLOW);
	s.type_builtin = SyntaxStyle::fg(YELLOW);
	s.type_parameter = SyntaxStyle::fg(YELLOW);
	s.type_enum_variant = SyntaxStyle::fg(CYAN);

	// Strings - cyan
	s.string = SyntaxStyle::fg(CYAN);
	s.string_regexp = SyntaxStyle::fg(RED);
	s.string_special = SyntaxStyle::fg(ORANGE);
	s.string_special_path = SyntaxStyle::fg(CYAN);
	s.string_special_url = SyntaxStyle::fg_mod(CYAN, Modifier::UNDERLINED);
	s.string_special_symbol = SyntaxStyle::fg(MAGENTA);

	// Constants - violet/magenta
	s.constant = SyntaxStyle::fg(CYAN);
	s.constant_builtin = SyntaxStyle::fg(VIOLET);
	s.constant_builtin_boolean = SyntaxStyle::fg(VIOLET);
	s.constant_character = SyntaxStyle::fg(CYAN);
	s.constant_character_escape = SyntaxStyle::fg(RED);
	s.constant_numeric = SyntaxStyle::fg(MAGENTA);
	s.constant_numeric_integer = SyntaxStyle::fg(MAGENTA);
	s.constant_numeric_float = SyntaxStyle::fg(MAGENTA);

	// Variables
	s.variable = SyntaxStyle::NONE;
	s.variable_builtin = SyntaxStyle::fg(ORANGE);
	s.variable_parameter = SyntaxStyle::NONE;
	s.variable_other = SyntaxStyle::NONE;
	s.variable_other_member = SyntaxStyle::NONE;

	// Operators and punctuation
	s.operator = SyntaxStyle::fg(GREEN);
	s.punctuation = SyntaxStyle::NONE;
	s.punctuation_bracket = SyntaxStyle::NONE;
	s.punctuation_delimiter = SyntaxStyle::NONE;
	s.punctuation_special = SyntaxStyle::fg(RED);

	// Other
	s.attribute = SyntaxStyle::fg(ORANGE);
	s.tag = SyntaxStyle::fg(BLUE);
	s.namespace = SyntaxStyle::fg(ORANGE);
	s.constructor = SyntaxStyle::fg(YELLOW);
	s.label = SyntaxStyle::fg(GREEN);
	s.special = SyntaxStyle::fg(ORANGE);

	// Markup
	s.markup_heading = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_heading_1 = SyntaxStyle::fg_mod(RED, Modifier::BOLD);
	s.markup_heading_2 = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_heading_3 = SyntaxStyle::fg_mod(YELLOW, Modifier::BOLD);
	s.markup_bold = SyntaxStyle::fg_mod(ORANGE, Modifier::BOLD);
	s.markup_italic = SyntaxStyle::fg_mod(VIOLET, Modifier::ITALIC);
	s.markup_strikethrough = SyntaxStyle::fg_mod(BASE01, Modifier::CROSSED_OUT);
	s.markup_link = SyntaxStyle::fg(VIOLET);
	s.markup_link_url = SyntaxStyle::fg_mod(CYAN, Modifier::UNDERLINED);
	s.markup_link_text = SyntaxStyle::fg(BLUE);
	s.markup_quote = SyntaxStyle::fg_mod(CYAN, Modifier::ITALIC);
	s.markup_raw = SyntaxStyle::fg(CYAN);
	s.markup_raw_inline = SyntaxStyle::fg(CYAN);
	s.markup_raw_block = SyntaxStyle::fg(CYAN);
	s.markup_list = SyntaxStyle::fg(MAGENTA);

	// Diff
	s.diff_plus = SyntaxStyle::fg(GREEN);
	s.diff_minus = SyntaxStyle::fg(RED);
	s.diff_delta = SyntaxStyle::fg(YELLOW);

	s
}
