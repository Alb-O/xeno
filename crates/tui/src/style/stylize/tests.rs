use alloc::format;

use itertools::Itertools;
use rstest::rstest;

use super::*;

#[test]
fn str_styled() {
	assert_eq!("hello".style(), Style::default());
	assert_eq!(
		"hello".set_style(Style::new().cyan()),
		Span::styled("hello", Style::new().cyan())
	);
	assert_eq!("hello".black(), Span::from("hello").black());
	assert_eq!("hello".red(), Span::from("hello").red());
	assert_eq!("hello".green(), Span::from("hello").green());
	assert_eq!("hello".yellow(), Span::from("hello").yellow());
	assert_eq!("hello".blue(), Span::from("hello").blue());
	assert_eq!("hello".magenta(), Span::from("hello").magenta());
	assert_eq!("hello".cyan(), Span::from("hello").cyan());
	assert_eq!("hello".gray(), Span::from("hello").gray());
	assert_eq!("hello".dark_gray(), Span::from("hello").dark_gray());
	assert_eq!("hello".light_red(), Span::from("hello").light_red());
	assert_eq!("hello".light_green(), Span::from("hello").light_green());
	assert_eq!("hello".light_yellow(), Span::from("hello").light_yellow());
	assert_eq!("hello".light_blue(), Span::from("hello").light_blue());
	assert_eq!("hello".light_magenta(), Span::from("hello").light_magenta());
	assert_eq!("hello".light_cyan(), Span::from("hello").light_cyan());
	assert_eq!("hello".white(), Span::from("hello").white());

	assert_eq!("hello".on_black(), Span::from("hello").on_black());
	assert_eq!("hello".on_red(), Span::from("hello").on_red());
	assert_eq!("hello".on_green(), Span::from("hello").on_green());
	assert_eq!("hello".on_yellow(), Span::from("hello").on_yellow());
	assert_eq!("hello".on_blue(), Span::from("hello").on_blue());
	assert_eq!("hello".on_magenta(), Span::from("hello").on_magenta());
	assert_eq!("hello".on_cyan(), Span::from("hello").on_cyan());
	assert_eq!("hello".on_gray(), Span::from("hello").on_gray());
	assert_eq!("hello".on_dark_gray(), Span::from("hello").on_dark_gray());
	assert_eq!("hello".on_light_red(), Span::from("hello").on_light_red());
	assert_eq!(
		"hello".on_light_green(),
		Span::from("hello").on_light_green()
	);
	assert_eq!(
		"hello".on_light_yellow(),
		Span::from("hello").on_light_yellow()
	);
	assert_eq!("hello".on_light_blue(), Span::from("hello").on_light_blue());
	assert_eq!(
		"hello".on_light_magenta(),
		Span::from("hello").on_light_magenta()
	);
	assert_eq!("hello".on_light_cyan(), Span::from("hello").on_light_cyan());
	assert_eq!("hello".on_white(), Span::from("hello").on_white());

	assert_eq!("hello".bold(), Span::from("hello").bold());
	assert_eq!("hello".dim(), Span::from("hello").dim());
	assert_eq!("hello".italic(), Span::from("hello").italic());
	assert_eq!("hello".underlined(), Span::from("hello").underlined());
	assert_eq!("hello".slow_blink(), Span::from("hello").slow_blink());
	assert_eq!("hello".rapid_blink(), Span::from("hello").rapid_blink());
	assert_eq!("hello".reversed(), Span::from("hello").reversed());
	assert_eq!("hello".hidden(), Span::from("hello").hidden());
	assert_eq!("hello".crossed_out(), Span::from("hello").crossed_out());

	assert_eq!("hello".not_bold(), Span::from("hello").not_bold());
	assert_eq!("hello".not_dim(), Span::from("hello").not_dim());
	assert_eq!("hello".not_italic(), Span::from("hello").not_italic());
	assert_eq!(
		"hello".not_underlined(),
		Span::from("hello").not_underlined()
	);
	assert_eq!(
		"hello".not_slow_blink(),
		Span::from("hello").not_slow_blink()
	);
	assert_eq!(
		"hello".not_rapid_blink(),
		Span::from("hello").not_rapid_blink()
	);
	assert_eq!("hello".not_reversed(), Span::from("hello").not_reversed());
	assert_eq!("hello".not_hidden(), Span::from("hello").not_hidden());
	assert_eq!(
		"hello".not_crossed_out(),
		Span::from("hello").not_crossed_out()
	);

	assert_eq!("hello".reset(), Span::from("hello").reset());
}

#[test]
fn string_styled() {
	let s = String::from("hello");
	assert_eq!(s.style(), Style::default());
	assert_eq!(
		s.clone().set_style(Style::new().cyan()),
		Span::styled("hello", Style::new().cyan())
	);
	assert_eq!(s.clone().black(), Span::from("hello").black());
	assert_eq!(s.clone().on_black(), Span::from("hello").on_black());
	assert_eq!(s.clone().bold(), Span::from("hello").bold());
	assert_eq!(s.clone().not_bold(), Span::from("hello").not_bold());
	assert_eq!(s.clone().reset(), Span::from("hello").reset());
}

#[test]
fn cow_string_styled() {
	let s = Cow::Borrowed("a");
	assert_eq!(s.red(), "a".red());
}

#[test]
fn temporary_string_styled() {
	// to_string() is used to create a temporary String, which is then styled. Without the
	// `Styled` trait impl for `String`, this would fail to compile with the error: "temporary
	// value dropped while borrowed"
	let s = "hello".to_string().red();
	assert_eq!(s, Span::from("hello").red());

	// format!() is used to create a temporary String inside a closure, which suffers the same
	// issue as above without the `Styled` trait impl for `String`
	let items = [String::from("a"), String::from("b")];
	let sss = items.iter().map(|s| format!("{s}{s}").red()).collect_vec();
	assert_eq!(sss, [Span::from("aa").red(), Span::from("bb").red()]);
}

#[test]
fn other_primitives_styled() {
	assert_eq!(true.red(), "true".red());
	assert_eq!('a'.red(), "a".red());
	assert_eq!(0.1f32.red(), "0.1".red());
	assert_eq!(0.1f64.red(), "0.1".red());
	assert_eq!(0i8.red(), "0".red());
	assert_eq!(0i16.red(), "0".red());
	assert_eq!(0i32.red(), "0".red());
	assert_eq!(0i64.red(), "0".red());
	assert_eq!(0i128.red(), "0".red());
	assert_eq!(0isize.red(), "0".red());
	assert_eq!(0u8.red(), "0".red());
	assert_eq!(0u16.red(), "0".red());
	assert_eq!(0u32.red(), "0".red());
	assert_eq!(0u64.red(), "0".red());
	assert_eq!(0u64.red(), "0".red());
	assert_eq!(0usize.red(), "0".red());
}

#[test]
fn reset() {
	assert_eq!(
		"hello".on_cyan().light_red().bold().underlined().reset(),
		Span::styled("hello", Style::reset())
	);
}

#[test]
fn fg() {
	let cyan_fg = Style::default().fg(Color::Cyan);

	assert_eq!("hello".cyan(), Span::styled("hello", cyan_fg));
}

#[test]
fn bg() {
	let cyan_bg = Style::default().bg(Color::Cyan);

	assert_eq!("hello".on_cyan(), Span::styled("hello", cyan_bg));
}

#[test]
fn color_modifier() {
	let cyan_bold = Style::default()
		.fg(Color::Cyan)
		.add_modifier(Modifier::BOLD);

	assert_eq!("hello".cyan().bold(), Span::styled("hello", cyan_bold));
}

#[test]
fn fg_bg() {
	let cyan_fg_bg = Style::default().bg(Color::Cyan).fg(Color::Cyan);

	assert_eq!("hello".cyan().on_cyan(), Span::styled("hello", cyan_fg_bg));
}

#[test]
fn repeated_attributes() {
	let bg = Style::default().bg(Color::Cyan);
	let fg = Style::default().fg(Color::Cyan);

	// Behavior: the last one set is the definitive one
	assert_eq!("hello".on_red().on_cyan(), Span::styled("hello", bg));
	assert_eq!("hello".red().cyan(), Span::styled("hello", fg));
}

#[test]
fn all_chained() {
	let all_modifier_black = Style::default()
		.bg(Color::Black)
		.fg(Color::Black)
		.add_modifier(
			Modifier::UNDERLINED
				| Modifier::BOLD
				| Modifier::DIM
				| Modifier::SLOW_BLINK
				| Modifier::REVERSED
				| Modifier::CROSSED_OUT,
		);
	assert_eq!(
		"hello"
			.on_black()
			.black()
			.bold()
			.underlined()
			.dim()
			.slow_blink()
			.crossed_out()
			.reversed(),
		Span::styled("hello", all_modifier_black)
	);
}

#[rstest]
#[case(Color::Black, ".black()")]
#[case(Color::Red, ".red()")]
#[case(Color::Green, ".green()")]
#[case(Color::Yellow, ".yellow()")]
#[case(Color::Blue, ".blue()")]
#[case(Color::Magenta, ".magenta()")]
#[case(Color::Cyan, ".cyan()")]
#[case(Color::Gray, ".gray()")]
#[case(Color::DarkGray, ".dark_gray()")]
#[case(Color::LightRed, ".light_red()")]
#[case(Color::LightGreen, ".light_green()")]
#[case(Color::LightYellow, ".light_yellow()")]
#[case(Color::LightBlue, ".light_blue()")]
#[case(Color::LightMagenta, ".light_magenta()")]
#[case(Color::LightCyan, ".light_cyan()")]
#[case(Color::White, ".white()")]
#[case(Color::Indexed(10), ".fg(Color::Indexed(10))")]
#[case(Color::Rgb(255, 0, 0), ".fg(Color::Rgb(255, 0, 0))")]
fn stylize_debug_foreground(#[case] color: Color, #[case] expected: &str) {
	let debug = color.stylize_debug(ColorDebugKind::Foreground);
	assert_eq!(format!("{debug:?}"), expected);
}

#[rstest]
#[case(Color::Black, ".on_black()")]
#[case(Color::Red, ".on_red()")]
#[case(Color::Green, ".on_green()")]
#[case(Color::Yellow, ".on_yellow()")]
#[case(Color::Blue, ".on_blue()")]
#[case(Color::Magenta, ".on_magenta()")]
#[case(Color::Cyan, ".on_cyan()")]
#[case(Color::Gray, ".on_gray()")]
#[case(Color::DarkGray, ".on_dark_gray()")]
#[case(Color::LightRed, ".on_light_red()")]
#[case(Color::LightGreen, ".on_light_green()")]
#[case(Color::LightYellow, ".on_light_yellow()")]
#[case(Color::LightBlue, ".on_light_blue()")]
#[case(Color::LightMagenta, ".on_light_magenta()")]
#[case(Color::LightCyan, ".on_light_cyan()")]
#[case(Color::White, ".on_white()")]
#[case(Color::Indexed(10), ".bg(Color::Indexed(10))")]
#[case(Color::Rgb(255, 0, 0), ".bg(Color::Rgb(255, 0, 0))")]
fn stylize_debug_background(#[case] color: Color, #[case] expected: &str) {
	let debug = color.stylize_debug(ColorDebugKind::Background);
	assert_eq!(format!("{debug:?}"), expected);
}

#[cfg(feature = "underline-color")]
#[rstest]
#[case(Color::Black, ".underline_color(Color::Black)")]
#[case(Color::Red, ".underline_color(Color::Red)")]
#[case(Color::Green, ".underline_color(Color::Green)")]
#[case(Color::Yellow, ".underline_color(Color::Yellow)")]
#[case(Color::Blue, ".underline_color(Color::Blue)")]
#[case(Color::Magenta, ".underline_color(Color::Magenta)")]
#[case(Color::Cyan, ".underline_color(Color::Cyan)")]
#[case(Color::Gray, ".underline_color(Color::Gray)")]
#[case(Color::DarkGray, ".underline_color(Color::DarkGray)")]
#[case(Color::LightRed, ".underline_color(Color::LightRed)")]
#[case(Color::LightGreen, ".underline_color(Color::LightGreen)")]
#[case(Color::LightYellow, ".underline_color(Color::LightYellow)")]
#[case(Color::LightBlue, ".underline_color(Color::LightBlue)")]
#[case(Color::LightMagenta, ".underline_color(Color::LightMagenta)")]
#[case(Color::LightCyan, ".underline_color(Color::LightCyan)")]
#[case(Color::White, ".underline_color(Color::White)")]
#[case(Color::Indexed(10), ".underline_color(Color::Indexed(10))")]
#[case(Color::Rgb(255, 0, 0), ".underline_color(Color::Rgb(255, 0, 0))")]
fn stylize_debug_underline(#[case] color: Color, #[case] expected: &str) {
	let debug = color.stylize_debug(ColorDebugKind::Underline);
	assert_eq!(format!("{debug:?}"), expected);
}
