use alloc::format;

use rstest::rstest;

use super::*;

#[rstest]
#[case(Style::new(), "Style::new()")]
#[case(Style::default(), "Style::new()")]
#[case(Style::new().red(), "Style::new().red()")]
#[case(Style::new().on_blue(), "Style::new().on_blue()")]
#[case(Style::new().bold(), "Style::new().bold()")]
#[case(Style::new().not_italic(), "Style::new().not_italic()")]
#[case(
	Style::new().red().on_blue().bold().italic().not_dim().not_hidden(),
	"Style::new().red().on_blue().bold().italic().not_dim().not_hidden()"
)]
fn debug(#[case] style: Style, #[case] expected: &'static str) {
	assert_eq!(format!("{style:?}"), expected);
}

#[test]
fn combined_patch_gives_same_result_as_individual_patch() {
	let styles = [
		Style::new(),
		Style::new().fg(Color::Yellow),
		Style::new().bg(Color::Yellow),
		Style::new().add_modifier(Modifier::BOLD),
		Style::new().remove_modifier(Modifier::BOLD),
		Style::new().add_modifier(Modifier::ITALIC),
		Style::new().remove_modifier(Modifier::ITALIC),
		Style::new().add_modifier(Modifier::ITALIC | Modifier::BOLD),
		Style::new().remove_modifier(Modifier::ITALIC | Modifier::BOLD),
	];
	for &a in &styles {
		for &b in &styles {
			for &c in &styles {
				for &d in &styles {
					assert_eq!(
						Style::new().patch(a).patch(b).patch(c).patch(d),
						Style::new().patch(a.patch(b.patch(c.patch(d))))
					);
				}
			}
		}
	}
}

#[test]
fn combine_individual_modifiers() {
	use crate::buffer::Buffer;
	use crate::layout::Rect;

	let mods = [
		Modifier::BOLD,
		Modifier::DIM,
		Modifier::ITALIC,
		Modifier::UNDERLINED,
		Modifier::SLOW_BLINK,
		Modifier::RAPID_BLINK,
		Modifier::REVERSED,
		Modifier::HIDDEN,
		Modifier::CROSSED_OUT,
	];

	let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));

	for m in mods {
		buffer[(0, 0)].set_style(Style::reset());
		buffer[(0, 0)].set_style(Style::new().add_modifier(m));
		let style = buffer[(0, 0)].style();
		assert!(style.add_modifier.contains(m));
		assert!(!style.sub_modifier.contains(m));
	}
}

#[rstest]
#[case(Modifier::empty(), "NONE")]
#[case(Modifier::BOLD, "BOLD")]
#[case(Modifier::DIM, "DIM")]
#[case(Modifier::ITALIC, "ITALIC")]
#[case(Modifier::UNDERLINED, "UNDERLINED")]
#[case(Modifier::SLOW_BLINK, "SLOW_BLINK")]
#[case(Modifier::RAPID_BLINK, "RAPID_BLINK")]
#[case(Modifier::REVERSED, "REVERSED")]
#[case(Modifier::HIDDEN, "HIDDEN")]
#[case(Modifier::CROSSED_OUT, "CROSSED_OUT")]
#[case(Modifier::BOLD | Modifier::DIM, "BOLD | DIM")]
#[case(
	Modifier::all(),
	"BOLD | DIM | ITALIC | UNDERLINED | SLOW_BLINK | RAPID_BLINK | REVERSED | HIDDEN | CROSSED_OUT"
)]
fn modifier_debug(#[case] modifier: Modifier, #[case] expected: &str) {
	assert_eq!(format!("{modifier:?}"), expected);
}

#[test]
fn style_can_be_const() {
	const RED: Color = Color::Red;
	const BLACK: Color = Color::Black;
	const BOLD: Modifier = Modifier::BOLD;
	const ITALIC: Modifier = Modifier::ITALIC;

	const _RESET: Style = Style::reset();
	const _RED_FG: Style = Style::new().fg(RED);
	const _RED_FG_SHORT: Style = Style::new().red();
	const _BLACK_BG: Style = Style::new().bg(BLACK);
	const _BLACK_BG_SHORT: Style = Style::new().on_black();
	const _ADD_BOLD: Style = Style::new().add_modifier(BOLD);
	const _ADD_BOLD_SHORT: Style = Style::new().bold();
	const _REMOVE_ITALIC: Style = Style::new().remove_modifier(ITALIC);
	const _REMOVE_ITALIC_SHORT: Style = Style::new().not_italic();
	const ALL: Style = Style::new()
		.fg(RED)
		.bg(BLACK)
		.add_modifier(BOLD)
		.remove_modifier(ITALIC);
	const ALL_SHORT: Style = Style::new().red().on_black().bold().not_italic();
	assert_eq!(
		ALL,
		Style::new()
			.fg(Color::Red)
			.bg(Color::Black)
			.add_modifier(Modifier::BOLD)
			.remove_modifier(Modifier::ITALIC)
	);
	assert_eq!(ALL, ALL_SHORT);
}

#[test]
fn has_modifier_checks() {
	// basic presence
	let style = Style::new().add_modifier(Modifier::BOLD | Modifier::ITALIC);
	assert!(style.has_modifier(Modifier::BOLD));
	assert!(style.has_modifier(Modifier::ITALIC));
	assert!(!style.has_modifier(Modifier::UNDERLINED));

	// removal prevents the modifier from being reported as present
	let style = Style::new()
		.add_modifier(Modifier::BOLD | Modifier::ITALIC)
		.remove_modifier(Modifier::ITALIC);
	assert!(style.has_modifier(Modifier::BOLD));
	assert!(!style.has_modifier(Modifier::ITALIC));

	// patching with a style that removes a modifier clears it
	let style = Style::new().add_modifier(Modifier::BOLD | Modifier::ITALIC);
	let patched = style.patch(Style::new().remove_modifier(Modifier::ITALIC));
	assert!(patched.has_modifier(Modifier::BOLD));
	assert!(!patched.has_modifier(Modifier::ITALIC));
}

#[rstest]
#[case(Style::new().black(), Color::Black)]
#[case(Style::new().red(), Color::Red)]
#[case(Style::new().green(), Color::Green)]
#[case(Style::new().yellow(), Color::Yellow)]
#[case(Style::new().blue(), Color::Blue)]
#[case(Style::new().magenta(), Color::Magenta)]
#[case(Style::new().cyan(), Color::Cyan)]
#[case(Style::new().white(), Color::White)]
#[case(Style::new().gray(), Color::Gray)]
#[case(Style::new().dark_gray(), Color::DarkGray)]
#[case(Style::new().light_red(), Color::LightRed)]
#[case(Style::new().light_green(), Color::LightGreen)]
#[case(Style::new().light_yellow(), Color::LightYellow)]
#[case(Style::new().light_blue(), Color::LightBlue)]
#[case(Style::new().light_magenta(), Color::LightMagenta)]
#[case(Style::new().light_cyan(), Color::LightCyan)]
#[case(Style::new().white(), Color::White)]
fn fg_can_be_stylized(#[case] stylized: Style, #[case] expected: Color) {
	assert_eq!(stylized, Style::new().fg(expected));
}

#[rstest]
#[case(Style::new().on_black(), Color::Black)]
#[case(Style::new().on_red(), Color::Red)]
#[case(Style::new().on_green(), Color::Green)]
#[case(Style::new().on_yellow(), Color::Yellow)]
#[case(Style::new().on_blue(), Color::Blue)]
#[case(Style::new().on_magenta(), Color::Magenta)]
#[case(Style::new().on_cyan(), Color::Cyan)]
#[case(Style::new().on_white(), Color::White)]
#[case(Style::new().on_gray(), Color::Gray)]
#[case(Style::new().on_dark_gray(), Color::DarkGray)]
#[case(Style::new().on_light_red(), Color::LightRed)]
#[case(Style::new().on_light_green(), Color::LightGreen)]
#[case(Style::new().on_light_yellow(), Color::LightYellow)]
#[case(Style::new().on_light_blue(), Color::LightBlue)]
#[case(Style::new().on_light_magenta(), Color::LightMagenta)]
#[case(Style::new().on_light_cyan(), Color::LightCyan)]
#[case(Style::new().on_white(), Color::White)]
fn bg_can_be_stylized(#[case] stylized: Style, #[case] expected: Color) {
	assert_eq!(stylized, Style::new().bg(expected));
}

#[rstest]
#[case(Style::new().bold(), Modifier::BOLD)]
#[case(Style::new().dim(), Modifier::DIM)]
#[case(Style::new().italic(), Modifier::ITALIC)]
#[case(Style::new().underlined(), Modifier::UNDERLINED)]
#[case(Style::new().slow_blink(), Modifier::SLOW_BLINK)]
#[case(Style::new().rapid_blink(), Modifier::RAPID_BLINK)]
#[case(Style::new().reversed(), Modifier::REVERSED)]
#[case(Style::new().hidden(), Modifier::HIDDEN)]
#[case(Style::new().crossed_out(), Modifier::CROSSED_OUT)]
fn add_modifier_can_be_stylized(#[case] stylized: Style, #[case] expected: Modifier) {
	assert_eq!(stylized, Style::new().add_modifier(expected));
}

#[rstest]
#[case(Style::new().not_bold(), Modifier::BOLD)]
#[case(Style::new().not_dim(), Modifier::DIM)]
#[case(Style::new().not_italic(), Modifier::ITALIC)]
#[case(Style::new().not_underlined(), Modifier::UNDERLINED)]
#[case(Style::new().not_slow_blink(), Modifier::SLOW_BLINK)]
#[case(Style::new().not_rapid_blink(), Modifier::RAPID_BLINK)]
#[case(Style::new().not_reversed(), Modifier::REVERSED)]
#[case(Style::new().not_hidden(), Modifier::HIDDEN)]
#[case(Style::new().not_crossed_out(), Modifier::CROSSED_OUT)]
fn remove_modifier_can_be_stylized(#[case] stylized: Style, #[case] expected: Modifier) {
	assert_eq!(stylized, Style::new().remove_modifier(expected));
}

#[test]
fn from_color() {
	assert_eq!(Style::from(Color::Red), Style::new().fg(Color::Red));
}

#[test]
fn from_color_color() {
	assert_eq!(
		Style::from((Color::Red, Color::Blue)),
		Style::new().fg(Color::Red).bg(Color::Blue)
	);
}

#[test]
fn from_modifier() {
	assert_eq!(
		Style::from(Modifier::BOLD | Modifier::ITALIC),
		Style::new()
			.add_modifier(Modifier::BOLD)
			.add_modifier(Modifier::ITALIC)
	);
}

#[test]
fn from_modifier_modifier() {
	assert_eq!(
		Style::from((Modifier::BOLD | Modifier::ITALIC, Modifier::DIM)),
		Style::new()
			.add_modifier(Modifier::BOLD)
			.add_modifier(Modifier::ITALIC)
			.remove_modifier(Modifier::DIM)
	);
}

#[test]
fn from_color_modifier() {
	assert_eq!(
		Style::from((Color::Red, Modifier::BOLD | Modifier::ITALIC)),
		Style::new()
			.fg(Color::Red)
			.add_modifier(Modifier::BOLD)
			.add_modifier(Modifier::ITALIC)
	);
}

#[test]
fn from_color_color_modifier() {
	assert_eq!(
		Style::from((Color::Red, Color::Blue, Modifier::BOLD | Modifier::ITALIC)),
		Style::new()
			.fg(Color::Red)
			.bg(Color::Blue)
			.add_modifier(Modifier::BOLD)
			.add_modifier(Modifier::ITALIC)
	);
}

#[test]
fn from_color_color_modifier_modifier() {
	assert_eq!(
		Style::from((
			Color::Red,
			Color::Blue,
			Modifier::BOLD | Modifier::ITALIC,
			Modifier::DIM
		)),
		Style::new()
			.fg(Color::Red)
			.bg(Color::Blue)
			.add_modifier(Modifier::BOLD)
			.add_modifier(Modifier::ITALIC)
			.remove_modifier(Modifier::DIM)
	);
}

#[cfg(feature = "serde")]
#[test]
fn serialize_then_deserialize() {
	let style = Style {
		fg: Some(Color::Rgb(255, 0, 255)),
		bg: Some(Color::White),
		#[cfg(feature = "underline-color")]
		underline_color: Some(Color::Indexed(3)),
		#[cfg(feature = "underline-color")]
		underline_style: None,
		add_modifier: Modifier::UNDERLINED,
		sub_modifier: Modifier::CROSSED_OUT,
	};

	let json_str = serde_json::to_string(&style).unwrap();
	let json_value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

	let mut expected_json = serde_json::json!({
		"fg": "#FF00FF",
		"bg": "White",
		"add_modifier": "UNDERLINED",
		"sub_modifier": "CROSSED_OUT"
	});

	#[cfg(feature = "underline-color")]
	{
		expected_json
			.as_object_mut()
			.unwrap()
			.insert("underline_color".into(), "3".into());
	}

	assert_eq!(json_value, expected_json);

	let deserialized: Style = serde_json::from_str(&json_str).unwrap();
	assert_eq!(deserialized, style);
}

#[cfg(feature = "serde")]
#[test]
fn deserialize_defaults() {
	let style = Style {
		fg: None,
		bg: None,
		#[cfg(feature = "underline-color")]
		underline_color: None,
		#[cfg(feature = "underline-color")]
		underline_style: None,
		add_modifier: Modifier::empty(),
		sub_modifier: Modifier::empty(),
	};

	let json_str = serde_json::to_string(&style).unwrap();
	assert_eq!(json_str, "{}");

	let deserialized: Style = serde_json::from_str(&json_str).unwrap();
	assert_eq!(deserialized, style);
}

#[cfg(feature = "serde")]
#[test]
fn deserialize_null_modifiers() {
	let json_value = serde_json::json!({
		"add_modifier": serde_json::Value::Null,
		"sub_modifier": serde_json::Value::Null
	});
	let json_str = serde_json::to_string(&json_value).unwrap();

	let style: Style = serde_json::from_str(&json_str).unwrap();

	assert!(style.add_modifier.is_empty());
	assert!(style.sub_modifier.is_empty());
}
