//! Debug theme with predictable, distinct colors for integration testing.
//!
//! This theme uses simple RGB values that are easy to identify and assert against
//! in automated tests. Each UI element has a unique, memorable color value.
//!
//! # Color Scheme
//!
//! The theme uses a systematic approach where each color component is a multiple
//! of 50 for easy identification:
//!
//! - Background: RGB(0, 0, 0) - pure black
//! - Foreground: RGB(255, 255, 255) - pure white
//! - Separator normal: RGB(100, 100, 100) - mid gray
//! - Separator hover: RGB(200, 200, 200) - light gray
//! - Selection: RGB(0, 0, 200) - pure blue bg
//! - Cursor: RGB(255, 255, 0) - pure yellow

use linkme::distributed_slice;

use crate::{
	Color, NotificationColors, PopupColors, StatusColors, SyntaxStyles, THEMES, Theme, ThemeColors,
	ThemeVariant, UiColors,
};

/// Pure black background - RGB(0, 0, 0)
const BG: Color = Color::Rgb(0, 0, 0);

/// Pure white foreground - RGB(255, 255, 255)
const FG: Color = Color::Rgb(255, 255, 255);

/// Separator/gutter color (unhovered) - RGB(100, 100, 100)
/// This is the color separators and line numbers use in their normal state.
const GUTTER: Color = Color::Rgb(100, 100, 100);

/// Cursor foreground (also used for hovered separator fg) - RGB(50, 50, 50)
/// Dark color visible against light backgrounds.
const CURSOR_FG: Color = Color::Rgb(50, 50, 50);

/// Cursor background - RGB(255, 255, 0) pure yellow
const CURSOR_BG: Color = Color::Rgb(255, 255, 0);

/// Cursorline background - RGB(30, 30, 30) slightly lighter than bg
const CURSORLINE_BG: Color = Color::Rgb(30, 30, 30);

/// Selection background - RGB(0, 0, 200) blue
/// Also used for hovered separator background.
const SELECTION_BG: Color = Color::Rgb(0, 0, 200);

/// Selection foreground - RGB(255, 255, 255) white
const SELECTION_FG: Color = Color::Rgb(255, 255, 255);

// Status bar mode colors - each mode has a distinct primary color
const STATUS_NORMAL_BG: Color = Color::Rgb(0, 100, 200); // Blue
const STATUS_INSERT_BG: Color = Color::Rgb(0, 200, 0); // Green
const STATUS_GOTO_BG: Color = Color::Rgb(200, 0, 200); // Magenta
const STATUS_VIEW_BG: Color = Color::Rgb(200, 100, 0); // Orange
const STATUS_COMMAND_BG: Color = Color::Rgb(200, 200, 0); // Yellow
const STATUS_FG: Color = Color::Rgb(0, 0, 0); // Black text on colored bg

// Semantic colors
const WARNING: Color = Color::Rgb(255, 200, 0); // Yellow-orange
const ERROR: Color = Color::Rgb(255, 0, 0); // Pure red
const SUCCESS: Color = Color::Rgb(0, 255, 0); // Pure green
const DIM: Color = Color::Rgb(100, 100, 100); // Same as gutter

// Popup colors
const POPUP_BG: Color = Color::Rgb(20, 20, 20);
const POPUP_BORDER: Color = Color::Rgb(150, 150, 150);

#[distributed_slice(THEMES)]
pub static DEBUG: Theme = Theme {
	id: "debug",
	name: "debug",
	aliases: &["test"],
	variant: ThemeVariant::Dark,
	colors: ThemeColors {
		ui: UiColors {
			bg: BG,
			fg: FG,
			gutter_fg: GUTTER,
			cursor_bg: CURSOR_BG,
			cursor_fg: CURSOR_FG,
			cursorline_bg: CURSORLINE_BG,
			selection_bg: SELECTION_BG,
			selection_fg: SELECTION_FG,
			message_fg: WARNING,
			command_input_fg: FG,
		},
		status: StatusColors {
			normal_bg: STATUS_NORMAL_BG,
			normal_fg: STATUS_FG,
			insert_bg: STATUS_INSERT_BG,
			insert_fg: STATUS_FG,
			goto_bg: STATUS_GOTO_BG,
			goto_fg: STATUS_FG,
			view_bg: STATUS_VIEW_BG,
			view_fg: STATUS_FG,
			command_bg: STATUS_COMMAND_BG,
			command_fg: STATUS_FG,

			dim_fg: DIM,
			warning_fg: WARNING,
			error_fg: ERROR,
			success_fg: SUCCESS,
		},
		popup: PopupColors {
			bg: POPUP_BG,
			fg: FG,
			border: POPUP_BORDER,
			title: SUCCESS,
		},
		notification: NotificationColors::INHERITED,
		syntax: SyntaxStyles::minimal(),
	},
	priority: -100, // Low priority - not a "real" theme
	source: evildoer_manifest::RegistrySource::Builtin,
};

/// Known color values for the debug theme, useful for test assertions.
pub mod colors {
	/// Background color: RGB(0, 0, 0)
	pub const BG: (u8, u8, u8) = (0, 0, 0);

	/// Foreground color: RGB(255, 255, 255)
	pub const FG: (u8, u8, u8) = (255, 255, 255);

	/// Gutter/separator normal color: RGB(100, 100, 100)
	pub const GUTTER: (u8, u8, u8) = (100, 100, 100);

	/// Cursor foreground (hovered separator fg): RGB(50, 50, 50)
	pub const CURSOR_FG: (u8, u8, u8) = (50, 50, 50);

	/// Cursor background: RGB(255, 255, 0)
	pub const CURSOR_BG: (u8, u8, u8) = (255, 255, 0);

	/// Cursorline background: RGB(30, 30, 30)
	pub const CURSORLINE_BG: (u8, u8, u8) = (30, 30, 30);

	/// Selection background (hovered separator bg): RGB(0, 0, 200)
	pub const SELECTION_BG: (u8, u8, u8) = (0, 0, 200);

	/// Selection foreground: RGB(255, 255, 255)
	pub const SELECTION_FG: (u8, u8, u8) = (255, 255, 255);

	/// Status bar normal mode: RGB(0, 100, 200)
	pub const STATUS_NORMAL: (u8, u8, u8) = (0, 100, 200);

	/// Status bar insert mode: RGB(0, 200, 0)
	pub const STATUS_INSERT: (u8, u8, u8) = (0, 200, 0);

	/// Warning color: RGB(255, 200, 0)
	pub const WARNING: (u8, u8, u8) = (255, 200, 0);

	/// Error color: RGB(255, 0, 0)
	pub const ERROR: (u8, u8, u8) = (255, 0, 0);

	/// Success color: RGB(0, 255, 0)
	pub const SUCCESS: (u8, u8, u8) = (0, 255, 0);
}
