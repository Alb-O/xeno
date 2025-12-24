use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;
use tui_term::vt100;

pub struct ThemedVt100Terminal<'a> {
	screen: &'a vt100::Screen,
	base_style: Style,
}

impl<'a> ThemedVt100Terminal<'a> {
	pub fn new(screen: &'a vt100::Screen, base_style: Style) -> Self {
		Self { screen, base_style }
	}
}

fn map_vt_color(color: vt100::Color) -> Option<Color> {
	match color {
		vt100::Color::Default => None,
		vt100::Color::Idx(i) => Some(Color::Indexed(i)),
		vt100::Color::Rgb(r, g, b) => Some(Color::Rgb(r, g, b)),
	}
}

fn style_for_cell(base_style: Style, cell: &vt100::Cell) -> Style {
	let mut style = base_style;

	if let Some(fg) = map_vt_color(cell.fgcolor()) {
		style = style.fg(fg);
	}
	if let Some(bg) = map_vt_color(cell.bgcolor()) {
		style = style.bg(bg);
	}

	let mut mods = Modifier::empty();
	if cell.bold() {
		mods |= Modifier::BOLD;
	}
	if cell.italic() {
		mods |= Modifier::ITALIC;
	}
	if cell.underline() {
		mods |= Modifier::UNDERLINED;
	}
	style = style.add_modifier(mods);

	if cell.inverse() {
		let fg = style.fg;
		let bg = style.bg;
		style = style.fg(bg.unwrap_or(Color::Reset));
		style = style.bg(fg.unwrap_or(Color::Reset));
	}

	style
}

impl Widget for ThemedVt100Terminal<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		// Always paint the full area with the base style.
		for y in area.top()..area.bottom() {
			for x in area.left()..area.right() {
				buf[(x, y)].set_style(self.base_style);
			}
		}

		let (rows, cols) = self.screen.size();
		let rows = rows.min(area.height);
		let cols = cols.min(area.width);

		for row in 0..rows {
			for col in 0..cols {
				let Some(cell) = self.screen.cell(row, col) else {
					continue;
				};

				if cell.is_wide_continuation() {
					continue;
				}

				let x = area.x + col;
				let y = area.y + row;
				let style = style_for_cell(self.base_style, cell);

				let out = &mut buf[(x, y)];
				out.set_style(style);

				if cell.has_contents() {
					let contents = cell.contents();
					out.set_symbol(&contents);
				} else {
					out.set_symbol(" ");
				}

				if cell.is_wide() {
					let next_x = x.saturating_add(1);
					if next_x < area.right() {
						let cont = &mut buf[(next_x, y)];
						cont.set_style(style);
						cont.set_symbol(" ");
					}
				}
			}
		}
	}
}
