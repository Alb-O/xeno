//! Implementation of the `insert_before` functionality for inline viewports.

use super::{Terminal, Viewport};
use crate::backend::Backend;
use crate::buffer::{Buffer, Cell};
#[cfg(not(feature = "scrolling-regions"))]
use crate::layout::Position;
use crate::layout::Rect;

impl<B> Terminal<B>
where
	B: Backend,
{
	/// Insert some content before the current inline viewport. This has no effect when the
	/// viewport is not inline.
	///
	/// The `draw_fn` closure will be called to draw into a writable `Buffer` that is `height`
	/// lines tall. The content of that `Buffer` will then be inserted before the viewport.
	///
	/// If the viewport isn't yet at the bottom of the screen, inserted lines will push it towards
	/// the bottom. Once the viewport is at the bottom of the screen, inserted lines will scroll
	/// the area of the screen above the viewport upwards.
	///
	/// Before:
	/// ```ignore
	/// +---------------------+
	/// | pre-existing line 1 |
	/// | pre-existing line 2 |
	/// +---------------------+
	/// |       viewport      |
	/// +---------------------+
	/// |                     |
	/// |                     |
	/// +---------------------+
	/// ```
	///
	/// After inserting 2 lines:
	/// ```ignore
	/// +---------------------+
	/// | pre-existing line 1 |
	/// | pre-existing line 2 |
	/// |   inserted line 1   |
	/// |   inserted line 2   |
	/// +---------------------+
	/// |       viewport      |
	/// +---------------------+
	/// +---------------------+
	/// ```
	///
	/// After inserting 2 more lines:
	/// ```ignore
	/// +---------------------+
	/// | pre-existing line 2 |
	/// |   inserted line 1   |
	/// |   inserted line 2   |
	/// |   inserted line 3   |
	/// |   inserted line 4   |
	/// +---------------------+
	/// |       viewport      |
	/// +---------------------+
	/// ```
	///
	/// If more lines are inserted than there is space on the screen, then the top lines will go
	/// directly into the terminal's scrollback buffer. At the limit, if the viewport takes up the
	/// whole screen, all lines will be inserted directly into the scrollback buffer.
	///
	/// # Examples
	///
	/// ## Insert a single line before the current viewport
	///
	/// ```rust,ignore
	/// use evildoer_tui::{
	///     backend::TestBackend,
	///     style::{Color, Style},
	///     text::{Line, Span},
	///     widgets::{Paragraph, Widget},
	///     Terminal,
	/// };
	/// # let backend = TestBackend::new(10, 10);
	/// # let mut terminal = Terminal::new(backend).unwrap();
	/// terminal.insert_before(1, |buf| {
	///     Paragraph::new(Line::from(vec![
	///         Span::raw("This line will be added "),
	///         Span::styled("before", Style::default().fg(Color::Blue)),
	///         Span::raw(" the current viewport"),
	///     ]))
	///     .render(buf.area, buf);
	/// });
	/// ```
	pub fn insert_before<F>(&mut self, height: u16, draw_fn: F) -> Result<(), B::Error>
	where
		F: FnOnce(&mut Buffer),
	{
		match self.viewport {
			#[cfg(feature = "scrolling-regions")]
			Viewport::Inline(_) => self.insert_before_scrolling_regions(height, draw_fn),
			#[cfg(not(feature = "scrolling-regions"))]
			Viewport::Inline(_) => self.insert_before_no_scrolling_regions(height, draw_fn),
			_ => Ok(()),
		}
	}

	/// Implement `Self::insert_before` using standard backend capabilities.
	#[cfg(not(feature = "scrolling-regions"))]
	fn insert_before_no_scrolling_regions(
		&mut self,
		height: u16,
		draw_fn: impl FnOnce(&mut Buffer),
	) -> Result<(), B::Error> {
		// The approach of this function is to first render all of the lines to insert into a
		// temporary buffer, and then to loop drawing chunks from the buffer to the screen. drawing
		// this buffer onto the screen.
		let area = Rect {
			x: 0,
			y: 0,
			width: self.viewport_area.width,
			height,
		};
		let mut buffer = Buffer::empty(area);
		draw_fn(&mut buffer);
		let mut buffer = buffer.content.as_slice();

		// Use i32 variables so we don't have worry about overflowed u16s when adding, or about
		// negative results when subtracting.
		let mut drawn_height: i32 = self.viewport_area.top().into();
		let mut buffer_height: i32 = height.into();
		let viewport_height: i32 = self.viewport_area.height.into();
		let screen_height: i32 = self.last_known_area.height.into();

		// The algorithm here is to loop, drawing large chunks of text (up to a screen-full at a
		// time), until the remainder of the buffer plus the viewport fits on the screen. We choose
		// this loop condition because it guarantees that we can write the remainder of the buffer
		// with just one call to Self::draw_lines().
		while buffer_height + viewport_height > screen_height {
			// We will draw as much of the buffer as possible on this iteration in order to make
			// forward progress. So we have:
			//
			//     to_draw = min(buffer_height, screen_height)
			//
			// We may need to scroll the screen up to make room to draw. We choose the minimal
			// possible scroll amount so we don't end up with the viewport sitting in the middle of
			// the screen when this function is done. The amount to scroll by is:
			//
			//     scroll_up = max(0, drawn_height + to_draw - screen_height)
			//
			// We want `scroll_up` to be enough so that, after drawing, we have used the whole
			// screen (drawn_height - scroll_up + to_draw = screen_height). However, there might
			// already be enough room on the screen to draw without scrolling (drawn_height +
			// to_draw <= screen_height). In this case, we just don't scroll at all.
			let to_draw = buffer_height.min(screen_height);
			let scroll_up = 0.max(drawn_height + to_draw - screen_height);
			self.scroll_up(scroll_up as u16)?;
			buffer = self.draw_lines((drawn_height - scroll_up) as u16, to_draw as u16, buffer)?;
			drawn_height += to_draw - scroll_up;
			buffer_height -= to_draw;
		}

		// There is now enough room on the screen for the remaining buffer plus the viewport,
		// though we may still need to scroll up some of the existing text first. It's possible
		// that by this point we've drained the buffer, but we may still need to scroll up to make
		// room for the viewport.
		//
		// We want to scroll up the exact amount that will leave us completely filling the screen.
		// However, it's possible that the viewport didn't start on the bottom of the screen and
		// the added lines weren't enough to push it all the way to the bottom. We deal with this
		// case by just ensuring that our scroll amount is non-negative.
		//
		// We want:
		//   screen_height = drawn_height - scroll_up + buffer_height + viewport_height
		// Or, equivalently:
		//   scroll_up = drawn_height + buffer_height + viewport_height - screen_height
		let scroll_up = 0.max(drawn_height + buffer_height + viewport_height - screen_height);
		self.scroll_up(scroll_up as u16)?;
		self.draw_lines(
			(drawn_height - scroll_up) as u16,
			buffer_height as u16,
			buffer,
		)?;
		drawn_height += buffer_height - scroll_up;

		self.set_viewport_area(Rect {
			y: drawn_height as u16,
			..self.viewport_area
		});

		// Clear the viewport off the screen. We didn't clear earlier for two reasons. First, it
		// wasn't necessary because the buffer we drew out of isn't sparse, so it overwrote
		// whatever was on the screen. Second, there is a weird bug with tmux where a full screen
		// clear plus immediate scrolling causes some garbage to go into the scrollback.
		self.clear()?;

		Ok(())
	}

	/// Implement `Self::insert_before` using scrolling regions.
	///
	/// If a terminal supports scrolling regions, it means that we can define a subset of rows of
	/// the screen, and then tell the terminal to scroll up or down just within that region. The
	/// rows outside of the region are not affected.
	///
	/// This function utilizes this feature to avoid having to redraw the viewport. This is done
	/// either by splitting the screen at the top of the viewport, and then creating a gap by
	/// either scrolling the viewport down, or scrolling the area above it up. The lines to insert
	/// are then drawn into the gap created.
	#[cfg(feature = "scrolling-regions")]
	fn insert_before_scrolling_regions(
		&mut self,
		mut height: u16,
		draw_fn: impl FnOnce(&mut Buffer),
	) -> Result<(), B::Error> {
		// The approach of this function is to first render all of the lines to insert into a
		// temporary buffer, and then to loop drawing chunks from the buffer to the screen. drawing
		// this buffer onto the screen.
		let area = Rect {
			x: 0,
			y: 0,
			width: self.viewport_area.width,
			height,
		};
		let mut buffer = Buffer::empty(area);
		draw_fn(&mut buffer);
		let mut buffer = buffer.content.as_slice();

		// Handle the special case where the viewport takes up the whole screen.
		if self.viewport_area.height == self.last_known_area.height {
			// "Borrow" the top line of the viewport. Draw over it, then immediately scroll it into
			// scrollback. Do this repeatedly until the whole buffer has been put into scrollback.
			let mut first = true;
			while !buffer.is_empty() {
				buffer = if first {
					self.draw_lines(0, 1, buffer)?
				} else {
					self.draw_lines_over_cleared(0, 1, buffer)?
				};
				first = false;
				self.backend.scroll_region_up(0..1, 1)?;
			}

			// Redraw the top line of the viewport.
			let width = self.viewport_area.width as usize;
			let top_line = self.buffers[1 - self.current].content[0..width].to_vec();
			self.draw_lines_over_cleared(0, 1, &top_line)?;
			return Ok(());
		}

		// Handle the case where the viewport isn't yet at the bottom of the screen.
		{
			let viewport_top = self.viewport_area.top();
			let viewport_bottom = self.viewport_area.bottom();
			let screen_bottom = self.last_known_area.bottom();
			if viewport_bottom < screen_bottom {
				let to_draw = height.min(screen_bottom - viewport_bottom);
				self.backend
					.scroll_region_down(viewport_top..viewport_bottom + to_draw, to_draw)?;
				buffer = self.draw_lines_over_cleared(viewport_top, to_draw, buffer)?;
				self.set_viewport_area(Rect {
					y: viewport_top + to_draw,
					..self.viewport_area
				});
				height -= to_draw;
			}
		}

		let viewport_top = self.viewport_area.top();
		while height > 0 {
			let to_draw = height.min(viewport_top);
			self.backend.scroll_region_up(0..viewport_top, to_draw)?;
			buffer = self.draw_lines_over_cleared(viewport_top - to_draw, to_draw, buffer)?;
			height -= to_draw;
		}

		Ok(())
	}

	/// Draw lines at the given vertical offset. The slice of cells must contain enough cells
	/// for the requested lines. A slice of the unused cells are returned.
	fn draw_lines<'a>(
		&mut self,
		y_offset: u16,
		lines_to_draw: u16,
		cells: &'a [Cell],
	) -> Result<&'a [Cell], B::Error> {
		let width: usize = self.last_known_area.width.into();
		let (to_draw, remainder) = cells.split_at(width * lines_to_draw as usize);
		if lines_to_draw > 0 {
			let iter = to_draw
				.iter()
				.enumerate()
				.map(|(i, c)| ((i % width) as u16, y_offset + (i / width) as u16, c));
			self.backend.draw(iter)?;
			self.backend.flush()?;
		}
		Ok(remainder)
	}

	/// Draw lines at the given vertical offset, assuming that the lines they are replacing on the
	/// screen are cleared. The slice of cells must contain enough cells for the requested lines. A
	/// slice of the unused cells are returned.
	#[cfg(feature = "scrolling-regions")]
	fn draw_lines_over_cleared<'a>(
		&mut self,
		y_offset: u16,
		lines_to_draw: u16,
		cells: &'a [Cell],
	) -> Result<&'a [Cell], B::Error> {
		let width: usize = self.last_known_area.width.into();
		let (to_draw, remainder) = cells.split_at(width * lines_to_draw as usize);
		if lines_to_draw > 0 {
			let area = Rect::new(0, y_offset, width as u16, y_offset + lines_to_draw);
			let old = Buffer::empty(area);
			let new = Buffer {
				area,
				content: to_draw.to_vec(),
			};
			self.backend.draw(old.diff(&new).into_iter())?;
			self.backend.flush()?;
		}
		Ok(remainder)
	}

	/// Scroll the whole screen up by the given number of lines.
	#[cfg(not(feature = "scrolling-regions"))]
	fn scroll_up(&mut self, lines_to_scroll: u16) -> Result<(), B::Error> {
		if lines_to_scroll > 0 {
			self.set_cursor_position(Position::new(
				0,
				self.last_known_area.height.saturating_sub(1),
			))?;
			self.backend.append_lines(lines_to_scroll)?;
		}
		Ok(())
	}
}
