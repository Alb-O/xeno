use rstest::rstest;

use super::*;

#[rstest]
#[case(CrosstermColor::Reset, Color::Reset)]
#[case(CrosstermColor::Black, Color::Black)]
#[case(CrosstermColor::DarkGrey, Color::DarkGray)]
#[case(CrosstermColor::Red, Color::LightRed)]
#[case(CrosstermColor::DarkRed, Color::Red)]
#[case(CrosstermColor::Green, Color::LightGreen)]
#[case(CrosstermColor::DarkGreen, Color::Green)]
#[case(CrosstermColor::Yellow, Color::LightYellow)]
#[case(CrosstermColor::DarkYellow, Color::Yellow)]
#[case(CrosstermColor::Blue, Color::LightBlue)]
#[case(CrosstermColor::DarkBlue, Color::Blue)]
#[case(CrosstermColor::Magenta, Color::LightMagenta)]
#[case(CrosstermColor::DarkMagenta, Color::Magenta)]
#[case(CrosstermColor::Cyan, Color::LightCyan)]
#[case(CrosstermColor::DarkCyan, Color::Cyan)]
#[case(CrosstermColor::White, Color::White)]
#[case(CrosstermColor::Grey, Color::Gray)]
#[case(CrosstermColor::Rgb { r: 0, g: 0, b: 0 }, Color::Rgb(0, 0, 0) )]
#[case(CrosstermColor::Rgb { r: 10, g: 20, b: 30 }, Color::Rgb(10, 20, 30) )]
#[case(CrosstermColor::AnsiValue(32), Color::Indexed(32))]
#[case(CrosstermColor::AnsiValue(37), Color::Indexed(37))]
fn from_crossterm_color(#[case] crossterm_color: CrosstermColor, #[case] color: Color) {
	assert_eq!(Color::from_crossterm(crossterm_color), color);
}

mod modifier {
	use super::*;

	#[rstest]
	#[case(CrosstermAttribute::Reset, Modifier::empty())]
	#[case(CrosstermAttribute::Bold, Modifier::BOLD)]
	#[case(CrosstermAttribute::NoBold, Modifier::empty())]
	#[case(CrosstermAttribute::Italic, Modifier::ITALIC)]
	#[case(CrosstermAttribute::NoItalic, Modifier::empty())]
	#[case(CrosstermAttribute::Underlined, Modifier::UNDERLINED)]
	#[case(CrosstermAttribute::NoUnderline, Modifier::empty())]
	#[case(CrosstermAttribute::OverLined, Modifier::empty())]
	#[case(CrosstermAttribute::NotOverLined, Modifier::empty())]
	#[case(CrosstermAttribute::DoubleUnderlined, Modifier::UNDERLINED)]
	#[case(CrosstermAttribute::Undercurled, Modifier::UNDERLINED)]
	#[case(CrosstermAttribute::Underdotted, Modifier::UNDERLINED)]
	#[case(CrosstermAttribute::Underdashed, Modifier::UNDERLINED)]
	#[case(CrosstermAttribute::Dim, Modifier::DIM)]
	#[case(CrosstermAttribute::NormalIntensity, Modifier::empty())]
	#[case(CrosstermAttribute::CrossedOut, Modifier::CROSSED_OUT)]
	#[case(CrosstermAttribute::NotCrossedOut, Modifier::empty())]
	#[case(CrosstermAttribute::NoUnderline, Modifier::empty())]
	#[case(CrosstermAttribute::SlowBlink, Modifier::SLOW_BLINK)]
	#[case(CrosstermAttribute::RapidBlink, Modifier::RAPID_BLINK)]
	#[case(CrosstermAttribute::Hidden, Modifier::HIDDEN)]
	#[case(CrosstermAttribute::NoHidden, Modifier::empty())]
	#[case(CrosstermAttribute::Reverse, Modifier::REVERSED)]
	#[case(CrosstermAttribute::NoReverse, Modifier::empty())]
	fn from_crossterm_attribute(
		#[case] crossterm_attribute: CrosstermAttribute,
		#[case] xeno_tui_modifier: Modifier,
	) {
		assert_eq!(
			Modifier::from_crossterm(crossterm_attribute),
			xeno_tui_modifier
		);
	}

	#[rstest]
	#[case(&[CrosstermAttribute::Bold], Modifier::BOLD)]
	#[case(&[CrosstermAttribute::Bold, CrosstermAttribute::Italic], Modifier::BOLD | Modifier::ITALIC)]
	#[case(&[CrosstermAttribute::Bold, CrosstermAttribute::NotCrossedOut], Modifier::BOLD)]
	#[case(&[CrosstermAttribute::Dim, CrosstermAttribute::Underdotted], Modifier::DIM | Modifier::UNDERLINED)]
	#[case(&[CrosstermAttribute::Dim, CrosstermAttribute::SlowBlink, CrosstermAttribute::Italic], Modifier::DIM | Modifier::SLOW_BLINK | Modifier::ITALIC)]
	#[case(&[CrosstermAttribute::Hidden, CrosstermAttribute::NoUnderline, CrosstermAttribute::NotCrossedOut], Modifier::HIDDEN)]
	#[case(&[CrosstermAttribute::Reverse], Modifier::REVERSED)]
	#[case(&[CrosstermAttribute::Reset], Modifier::empty())]
	#[case(&[CrosstermAttribute::RapidBlink, CrosstermAttribute::CrossedOut], Modifier::RAPID_BLINK | Modifier::CROSSED_OUT)]
	fn from_crossterm_attributes(
		#[case] crossterm_attributes: &[CrosstermAttribute],
		#[case] xeno_tui_modifier: Modifier,
	) {
		assert_eq!(
			Modifier::from_crossterm(CrosstermAttributes::from(crossterm_attributes)),
			xeno_tui_modifier
		);
	}
}

#[rstest]
#[case(ContentStyle::default(), Style::default())]
#[case(
        ContentStyle {
            foreground_color: Some(CrosstermColor::DarkYellow),
            ..Default::default()
        },
        Style::default().fg(Color::Yellow)
    )]
#[case(
        ContentStyle {
            background_color: Some(CrosstermColor::DarkYellow),
            ..Default::default()
        },
        Style::default().bg(Color::Yellow)
    )]
#[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(CrosstermAttribute::Bold),
            ..Default::default()
        },
        Style::default().add_modifier(Modifier::BOLD)
    )]
#[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(CrosstermAttribute::NoBold),
            ..Default::default()
        },
        Style::default().remove_modifier(Modifier::BOLD)
    )]
#[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(CrosstermAttribute::Italic),
            ..Default::default()
        },
        Style::default().add_modifier(Modifier::ITALIC)
    )]
#[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(CrosstermAttribute::NoItalic),
            ..Default::default()
        },
        Style::default().remove_modifier(Modifier::ITALIC)
    )]
#[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(
                [CrosstermAttribute::Bold, CrosstermAttribute::Italic].as_ref()
            ),
            ..Default::default()
        },
        Style::default()
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::ITALIC)
    )]
#[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(
                [CrosstermAttribute::NoBold, CrosstermAttribute::NoItalic].as_ref()
            ),
            ..Default::default()
        },
        Style::default()
            .remove_modifier(Modifier::BOLD)
            .remove_modifier(Modifier::ITALIC)
    )]
fn from_crossterm_content_style(#[case] content_style: ContentStyle, #[case] style: Style) {
	assert_eq!(Style::from_crossterm(content_style), style);
}

#[test]
#[cfg(feature = "underline-color")]
fn from_crossterm_content_style_underline() {
	let content_style = ContentStyle {
		underline_color: Some(CrosstermColor::DarkRed),
		..Default::default()
	};
	assert_eq!(
		Style::from_crossterm(content_style),
		Style::default().underline_color(Color::Red)
	);
}
