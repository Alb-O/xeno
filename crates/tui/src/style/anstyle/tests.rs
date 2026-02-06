use super::*;

#[test]
fn anstyle_to_color() {
	let anstyle_color = Ansi256Color(42);
	let color = Color::from(anstyle_color);
	assert_eq!(color, Color::Indexed(42));
}

#[test]
fn color_to_ansi256color() {
	let color = Color::Indexed(42);
	let anstyle_color = Ansi256Color::try_from(color);
	assert_eq!(anstyle_color, Ok(Ansi256Color(42)));
}

#[test]
fn color_to_ansi256color_error() {
	let color = Color::Rgb(0, 0, 0);
	let anstyle_color = Ansi256Color::try_from(color);
	assert_eq!(anstyle_color, Err(TryFromColorError::Ansi256));
}

#[test]
fn ansi_color_to_color() {
	let ansi_color = AnsiColor::Red;
	let color = Color::from(ansi_color);
	assert_eq!(color, Color::Red);
}

#[test]
fn color_to_ansicolor() {
	let color = Color::Red;
	let ansi_color = AnsiColor::try_from(color);
	assert_eq!(ansi_color, Ok(AnsiColor::Red));
}

#[test]
fn color_to_ansicolor_error() {
	let color = Color::Rgb(0, 0, 0);
	let ansi_color = AnsiColor::try_from(color);
	assert_eq!(ansi_color, Err(TryFromColorError::Ansi));
}

#[test]
fn rgb_color_to_color() {
	let rgb_color = RgbColor(255, 0, 0);
	let color = Color::from(rgb_color);
	assert_eq!(color, Color::Rgb(255, 0, 0));
}

#[test]
fn color_to_rgbcolor() {
	let color = Color::Rgb(255, 0, 0);
	let rgb_color = RgbColor::try_from(color);
	assert_eq!(rgb_color, Ok(RgbColor(255, 0, 0)));
}

#[test]
fn color_to_rgbcolor_error() {
	let color = Color::Indexed(42);
	let rgb_color = RgbColor::try_from(color);
	assert_eq!(rgb_color, Err(TryFromColorError::RgbColor));
}

#[test]
fn effects_to_modifier() {
	let effects = Effects::BOLD | Effects::ITALIC;
	let modifier = Modifier::from(effects);
	assert!(modifier.contains(Modifier::BOLD));
	assert!(modifier.contains(Modifier::ITALIC));
}

#[test]
fn modifier_to_effects() {
	let modifier = Modifier::BOLD | Modifier::ITALIC;
	let effects = Effects::from(modifier);
	assert!(effects.contains(Effects::BOLD));
	assert!(effects.contains(Effects::ITALIC));
}

#[test]
fn anstyle_style_to_style() {
	let anstyle_style = anstyle::Style::new()
		.fg_color(Some(anstyle::Color::Ansi(AnsiColor::Red)))
		.bg_color(Some(anstyle::Color::Ansi(AnsiColor::Blue)))
		.underline_color(Some(anstyle::Color::Ansi(AnsiColor::Green)))
		.effects(Effects::BOLD | Effects::ITALIC);
	let style = Style::from(anstyle_style);
	assert_eq!(style.fg, Some(Color::Red));
	assert_eq!(style.bg, Some(Color::Blue));
	#[cfg(feature = "underline-color")]
	assert_eq!(style.underline_color, Some(Color::Green));
	assert!(style.add_modifier.contains(Modifier::BOLD));
	assert!(style.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn style_to_anstyle_style() {
	let style = Style {
		fg: Some(Color::Red),
		bg: Some(Color::Blue),
		#[cfg(feature = "underline-color")]
		underline_color: Some(Color::Green),
		add_modifier: Modifier::BOLD | Modifier::ITALIC,
		..Default::default()
	};
	let anstyle_style = anstyle::Style::from(style);
	assert_eq!(
		anstyle_style.get_fg_color(),
		Some(anstyle::Color::Ansi(AnsiColor::Red))
	);
	assert_eq!(
		anstyle_style.get_bg_color(),
		Some(anstyle::Color::Ansi(AnsiColor::Blue))
	);
	#[cfg(feature = "underline-color")]
	assert_eq!(
		anstyle_style.get_underline_color(),
		Some(anstyle::Color::Ansi(AnsiColor::Green))
	);
	assert!(anstyle_style.get_effects().contains(Effects::BOLD));
	assert!(anstyle_style.get_effects().contains(Effects::ITALIC));
}
