use pretty_assertions::assert_eq;

use super::*;
use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::{Color, Modifier, Stylize};
use crate::text::{Text, ToSpan};
use crate::widgets::StatefulWidget;

#[test]
fn collect_list_from_iterator() {
	let collected: List = (0..3).map(|i| format!("Item{i}")).collect();
	let expected = List::new(["Item0", "Item1", "Item2"]);
	assert_eq!(collected, expected);
}

#[test]
fn can_be_stylized() {
	assert_eq!(
		List::new::<Vec<&str>>(vec![]).black().on_white().bold().not_dim().style,
		Style::default()
			.fg(Color::Black)
			.bg(Color::White)
			.add_modifier(Modifier::BOLD)
			.remove_modifier(Modifier::DIM)
	);
}

#[test]
fn no_style() {
	let text = Text::from("Item 1");
	let list = List::new([ListItem::new(text)])
		.highlight_symbol(">>")
		.highlight_spacing(HighlightSpacing::Always);
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));

	list.render(buffer.area, &mut buffer, &mut ListState::default());

	assert_eq!(buffer, Buffer::with_lines(["  Item 1  "]));
}

#[test]
fn styled_text() {
	let text = Text::from("Item 1").bold();
	let list = List::new([ListItem::new(text)])
		.highlight_symbol(">>")
		.highlight_spacing(HighlightSpacing::Always);
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));

	list.render(buffer.area, &mut buffer, &mut ListState::default());

	assert_eq!(buffer, Buffer::with_lines([Line::from(vec!["  ".to_span(), "Item 1  ".bold(),])]));
}

#[test]
fn styled_list_item() {
	let text = Text::from("Item 1");
	// note this avoids using the `Stylize' methods as that gets then combines the style
	// instead of setting it directly (which is not the same for some implementations)
	let item = ListItem::new(text).style(Modifier::ITALIC);
	let list = List::new([item]).highlight_symbol(">>").highlight_spacing(HighlightSpacing::Always);
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));

	list.render(buffer.area, &mut buffer, &mut ListState::default());

	assert_eq!(buffer, Buffer::with_lines([Line::from_iter(["  Item 1  ".italic()])]));
}

#[test]
fn styled_text_and_list_item() {
	let text = Text::from("Item 1").bold();
	// note this avoids using the `Stylize' methods as that gets then combines the style
	// instead of setting it directly (which is not the same for some implementations)
	let item = ListItem::new(text).style(Modifier::ITALIC);
	let list = List::new([item]).highlight_symbol(">>").highlight_spacing(HighlightSpacing::Always);
	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));

	list.render(buffer.area, &mut buffer, &mut ListState::default());

	assert_eq!(buffer, Buffer::with_lines([Line::from(vec!["  ".italic(), "Item 1  ".bold().italic()])]));
}

#[test]
fn styled_highlight() {
	let text = Text::from("Item 1").bold();
	// note this avoids using the `Stylize' methods as that gets then combines the style
	// instead of setting it directly (which is not the same for some implementations)
	let item = ListItem::new(text).style(Modifier::ITALIC);
	let mut state = ListState::default().with_selected(Some(0));
	let list = List::new([item]).highlight_symbol(">>").highlight_style(Color::Red);

	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 1));
	list.render(buffer.area, &mut buffer, &mut state);

	assert_eq!(
		buffer,
		Buffer::with_lines([Line::from(vec![">>".italic().red(), "Item 1  ".bold().italic().red(),])])
	);
}

#[test]
fn style_inheritance() {
	let bold = Modifier::BOLD;
	let italic = Modifier::ITALIC;
	let items = [
		ListItem::new(Text::raw("Item 1")),
		ListItem::new(Text::styled("Item 2", bold)),
		ListItem::new(Text::raw("Item 3")).style(italic),
		ListItem::new(Text::styled("Item 4", bold)).style(italic),
		ListItem::new(Text::styled("Item 5", bold)).style(italic),
	];
	let mut state = ListState::default().with_selected(Some(4));
	let list = List::new(items)
		.highlight_symbol(">>")
		.highlight_style(Color::Red)
		.style(Style::new().on_blue());

	let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
	list.render(buffer.area, &mut buffer, &mut state);

	assert_eq!(
		buffer,
		Buffer::with_lines(vec![
			vec!["  Item 1  ".on_blue()],
			vec!["  ".on_blue(), "Item 2  ".bold().on_blue()],
			vec!["  Item 3  ".italic().on_blue()],
			vec!["  ".italic().on_blue(), "Item 4  ".bold().italic().on_blue(),],
			vec![">>".italic().red().on_blue(), "Item 5  ".bold().italic().red().on_blue(),],
		])
	);
}

#[test]
fn render_in_minimal_buffer() {
	let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
	let mut state = ListState::default().with_selected(None);
	let items = vec![ListItem::new("Item 1"), ListItem::new("Item 2"), ListItem::new("Item 3")];
	let list = List::new(items);
	// This should not panic, even if the buffer is too small to render the list.
	list.render(buffer.area, &mut buffer, &mut state);
	assert_eq!(buffer, Buffer::with_lines(["I"]));
}

#[test]
fn render_in_zero_size_buffer() {
	let mut buffer = Buffer::empty(Rect::ZERO);
	let mut state = ListState::default().with_selected(None);
	let items = vec![ListItem::new("Item 1"), ListItem::new("Item 2"), ListItem::new("Item 3")];
	let list = List::new(items);
	// This should not panic, even if the buffer has zero size.
	list.render(buffer.area, &mut buffer, &mut state);
}
