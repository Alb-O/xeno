use super::*;

#[test]
fn new() {
	let cell = Cell::new("あ");
	assert_eq!(
		cell,
		Cell {
			symbol: Some(CompactString::const_new("あ")),
			fg: Color::Reset,
			bg: Color::Reset,
			#[cfg(feature = "underline-color")]
			underline_color: Color::Reset,
			#[cfg(feature = "underline-color")]
			underline_style: UnderlineStyle::Reset,
			modifier: Modifier::empty(),
			skip: false,
		}
	);
}

#[test]
fn empty() {
	let cell = Cell::EMPTY;
	assert_eq!(cell.symbol(), " ");
}

#[test]
fn set_symbol() {
	let mut cell = Cell::EMPTY;
	cell.set_symbol("あ");
	assert_eq!(cell.symbol(), "あ");
	cell.set_symbol("👨‍👩‍👧‍👦");
	assert_eq!(cell.symbol(), "👨‍👩‍👧‍👦");
}

#[test]
fn append_symbol() {
	let mut cell = Cell::EMPTY;
	cell.set_symbol("あ");
	cell.append_symbol("\u{200B}");
	assert_eq!(cell.symbol(), "あ\u{200B}");
}

#[test]
fn set_char() {
	let mut cell = Cell::EMPTY;
	cell.set_char('あ');
	assert_eq!(cell.symbol(), "あ");
}

#[test]
fn set_fg() {
	let mut cell = Cell::EMPTY;
	cell.set_fg(Color::Red);
	assert_eq!(cell.fg, Color::Red);
}

#[test]
fn set_bg() {
	let mut cell = Cell::EMPTY;
	cell.set_bg(Color::Red);
	assert_eq!(cell.bg, Color::Red);
}

#[test]
fn set_style() {
	let mut cell = Cell::EMPTY;
	cell.set_style(Style::new().fg(Color::Red).bg(Color::Blue));
	assert_eq!(cell.fg, Color::Red);
	assert_eq!(cell.bg, Color::Blue);
}

#[test]
fn set_skip() {
	let mut cell = Cell::EMPTY;
	cell.set_skip(true);
	assert!(cell.skip);
}

#[test]
fn reset() {
	let mut cell = Cell::EMPTY;
	cell.set_symbol("あ");
	cell.set_fg(Color::Red);
	cell.set_bg(Color::Blue);
	cell.set_skip(true);
	cell.reset();
	assert_eq!(cell.symbol(), " ");
	assert_eq!(cell.fg, Color::Reset);
	assert_eq!(cell.bg, Color::Reset);
	assert!(!cell.skip);
}

#[test]
fn style() {
	let cell = Cell::EMPTY;
	assert_eq!(
		cell.style(),
		Style {
			fg: Some(Color::Reset),
			bg: Some(Color::Reset),
			#[cfg(feature = "underline-color")]
			underline_color: Some(Color::Reset),
			#[cfg(feature = "underline-color")]
			underline_style: Some(UnderlineStyle::Reset),
			add_modifier: Modifier::empty(),
			sub_modifier: Modifier::empty(),
		}
	);
}

#[test]
fn default() {
	let cell = Cell::default();
	assert_eq!(cell.symbol(), " ");
}

#[test]
fn cell_eq() {
	let cell1 = Cell::new("あ");
	let cell2 = Cell::new("あ");
	assert_eq!(cell1, cell2);
}

#[test]
fn cell_ne() {
	let cell1 = Cell::new("あ");
	let cell2 = Cell::new("い");
	assert_ne!(cell1, cell2);
}
