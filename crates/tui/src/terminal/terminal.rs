use super::{CompletedFrame, Frame, Viewport};
use crate::backend::{Backend, ClearType};
use crate::buffer::{Buffer, DiffUpdate};
use crate::layout::{Position, Rect, Size};

/// Main interface for drawing to the terminal.
///
/// Maintains a double buffer: widgets draw to the current buffer, then at flush time only the
/// diff against the previous buffer is written to the backend. Buffers swap after each draw.
///
/// The viewport controls what portion of the terminal is used: fullscreen, inline (fixed height
/// below cursor), or fixed rect. Fullscreen/inline viewports auto-resize on terminal resize.
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Terminal<B>
where
	B: Backend,
{
	/// The backend used to communicate with the terminal.
	pub(super) backend: B,
	/// Double buffer: current and previous. Diffed at flush time.
	pub(super) buffers: [Buffer; 2],
	/// Index of the current buffer (0 or 1).
	pub(super) current: usize,
	/// Whether the cursor is currently hidden.
	hidden_cursor: bool,
	/// Viewport configuration determining which portion of terminal to use.
	pub(super) viewport: Viewport,
	/// The area of the viewport after accounting for inline offset.
	pub(super) viewport_area: Rect,
	/// Last known terminal size from backend query.
	pub(super) last_known_area: Rect,
	/// Last known cursor position after rendering.
	pub(super) last_known_cursor_pos: Position,
	/// Number of frames rendered (wraps on overflow).
	frame_count: usize,
	/// Reusable scratch buffer for diff output, avoiding per-frame allocation.
	diff_scratch: Vec<DiffUpdate>,
}

/// Options for [`Terminal::with_options`].
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct TerminalOptions {
	/// Viewport used to draw to the terminal.
	pub viewport: Viewport,
}

impl<B> Drop for Terminal<B>
where
	B: Backend,
{
	fn drop(&mut self) {
		if self.hidden_cursor
			&& let Err(err) = self.show_cursor()
		{
			eprintln!("Failed to show the cursor: {err}");
		}
	}
}

impl<B> Terminal<B>
where
	B: Backend,
{
	/// Creates a new terminal with fullscreen viewport.
	///
	/// Does not install a panic hook - if you panic while in alternate screen mode,
	/// the terminal may be left in an unusable state.
	pub fn new(backend: B) -> Result<Self, B::Error> {
		Self::with_options(
			backend,
			TerminalOptions {
				viewport: Viewport::Fullscreen,
			},
		)
	}

	/// Creates a new terminal with the given options.
	pub fn with_options(mut backend: B, options: TerminalOptions) -> Result<Self, B::Error> {
		let area = match options.viewport {
			Viewport::Fullscreen | Viewport::Inline(_) => backend.size()?.into(),
			Viewport::Fixed(area) => area,
		};
		let (viewport_area, cursor_pos) = match options.viewport {
			Viewport::Fullscreen => (area, Position::ORIGIN),
			Viewport::Inline(height) => compute_inline_size(&mut backend, height, area.as_size(), 0)?,
			Viewport::Fixed(area) => (area, area.as_position()),
		};
		Ok(Self {
			backend,
			buffers: [Buffer::empty(viewport_area), Buffer::empty(viewport_area)],
			current: 0,
			hidden_cursor: false,
			viewport: options.viewport,
			viewport_area,
			last_known_area: area,
			last_known_cursor_pos: cursor_pos,
			frame_count: 0,
			diff_scratch: Vec::new(),
		})
	}

	/// Returns a [`Frame`] for direct buffer access outside of [`draw`](Self::draw).
	///
	/// Useful for unit testing widgets or manual buffer manipulation. When using this,
	/// you must call [`flush`](Self::flush), [`swap_buffers`](Self::swap_buffers), and
	/// `backend.flush()` manually.
	pub const fn get_frame(&mut self) -> Frame<'_> {
		let count = self.frame_count;
		Frame {
			cursor_position: None,
			viewport_area: self.viewport_area,
			buffer: self.current_buffer_mut(),
			count,
		}
	}

	/// Returns mutable reference to the current buffer.
	pub const fn current_buffer_mut(&mut self) -> &mut Buffer {
		&mut self.buffers[self.current]
	}

	/// Returns the backend.
	pub const fn backend(&self) -> &B {
		&self.backend
	}

	/// Returns mutable reference to the backend.
	pub const fn backend_mut(&mut self) -> &mut B {
		&mut self.backend
	}

	/// Diffs current vs previous buffer and writes changes to the backend.
	///
	/// Uses a reusable scratch buffer for diff output to avoid per-frame allocation.
	pub fn flush(&mut self) -> Result<(), B::Error> {
		let previous_buffer = &self.buffers[1 - self.current];
		let current_buffer = &self.buffers[self.current];

		#[cfg(feature = "perf")]
		let t0 = std::time::Instant::now();

		previous_buffer.diff_into(current_buffer, &mut self.diff_scratch);

		#[cfg(feature = "perf")]
		{
			let ns = t0.elapsed().as_nanos() as u64;
			tracing::debug!(
				target: "perf",
				tui_diff_ns = ns,
				tui_diff_updates = self.diff_scratch.len() as u64,
			);
		}

		if let Some(last) = self.diff_scratch.last() {
			self.last_known_cursor_pos = Position { x: last.x, y: last.y };
		}
		self.backend.draw(self.diff_scratch.iter().map(|u| (u.x, u.y, &current_buffer.content[u.idx])))
	}

	/// Resizes internal buffers to match the given area. Clears the screen.
	pub fn resize(&mut self, area: Rect) -> Result<(), B::Error> {
		let next_area = match self.viewport {
			Viewport::Inline(height) => {
				let offset_in_previous_viewport = self.last_known_cursor_pos.y.saturating_sub(self.viewport_area.top());
				compute_inline_size(&mut self.backend, height, area.as_size(), offset_in_previous_viewport)?.0
			}
			Viewport::Fixed(_) | Viewport::Fullscreen => area,
		};
		self.set_viewport_area(next_area);
		self.clear()?;

		self.last_known_area = area;
		Ok(())
	}

	/// Updates the viewport area and resizes both buffers to match.
	pub(super) fn set_viewport_area(&mut self, area: Rect) {
		self.buffers[self.current].resize(area);
		self.buffers[1 - self.current].resize(area);
		self.viewport_area = area;
	}

	/// Queries backend size and resizes if changed. No-op for fixed viewports.
	pub fn autoresize(&mut self) -> Result<(), B::Error> {
		if matches!(self.viewport, Viewport::Fullscreen | Viewport::Inline(_)) {
			let area = self.size()?.into();
			if area != self.last_known_area {
				self.resize(area)?;
			}
		}
		Ok(())
	}

	/// Draws a frame. This is the main render entry point.
	///
	/// Handles autoresize, calls your render callback, flushes the diff to the backend,
	/// updates cursor visibility/position, and swaps buffers.
	///
	/// The callback must fully render the frame - unchanged areas are detected via diff,
	/// so partial renders will leave stale content.
	///
	/// Use [`try_draw`](Self::try_draw) if your callback can fail.
	pub fn draw<F>(&mut self, render_callback: F) -> Result<CompletedFrame<'_>, B::Error>
	where
		F: FnOnce(&mut Frame),
	{
		self.try_draw(|frame| {
			render_callback(frame);
			Ok::<(), B::Error>(())
		})
	}

	/// Like [`draw`](Self::draw), but the callback returns a `Result`.
	///
	/// Errors from the callback are converted via `Into<B::Error>`.
	pub fn try_draw<F, E>(&mut self, render_callback: F) -> Result<CompletedFrame<'_>, B::Error>
	where
		F: FnOnce(&mut Frame) -> Result<(), E>,
		E: Into<B::Error>,
	{
		self.autoresize()?;

		let mut frame = self.get_frame();
		render_callback(&mut frame).map_err(Into::into)?;

		// Extract cursor position before dropping frame (which borrows buffer)
		let cursor_position = frame.cursor_position;

		self.flush()?;

		match cursor_position {
			None => self.hide_cursor()?,
			Some(position) => {
				self.show_cursor()?;
				self.set_cursor_position(position)?;
			}
		}

		self.swap_buffers();
		self.backend.flush()?;

		let completed_frame = CompletedFrame {
			buffer: &self.buffers[1 - self.current],
			area: self.last_known_area,
			count: self.frame_count,
		};

		self.frame_count = self.frame_count.wrapping_add(1);

		Ok(completed_frame)
	}

	/// Hides the cursor.
	pub fn hide_cursor(&mut self) -> Result<(), B::Error> {
		self.backend.hide_cursor()?;
		self.hidden_cursor = true;
		Ok(())
	}

	/// Shows the cursor.
	pub fn show_cursor(&mut self) -> Result<(), B::Error> {
		self.backend.show_cursor()?;
		self.hidden_cursor = false;
		Ok(())
	}

	/// Gets cursor position from the backend.
	pub fn get_cursor_position(&mut self) -> Result<Position, B::Error> {
		self.backend.get_cursor_position()
	}

	/// Sets cursor position.
	pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), B::Error> {
		let position = position.into();
		self.backend.set_cursor_position(position)?;
		self.last_known_cursor_pos = position;
		Ok(())
	}

	/// Clears the terminal and forces a full redraw on next draw.
	pub fn clear(&mut self) -> Result<(), B::Error> {
		match self.viewport {
			Viewport::Fullscreen => self.backend.clear_region(ClearType::All)?,
			Viewport::Inline(_) => {
				self.backend.set_cursor_position(self.viewport_area.as_position())?;
				self.backend.clear_region(ClearType::AfterCursor)?;
			}
			Viewport::Fixed(_) => {
				let area = self.viewport_area;
				for y in area.top()..area.bottom() {
					self.backend.set_cursor_position(Position { x: 0, y })?;
					self.backend.clear_region(ClearType::AfterCursor)?;
				}
			}
		}
		// Reset back buffer so next draw will redraw everything
		self.buffers[1 - self.current].reset();
		Ok(())
	}

	/// Clears the inactive buffer and swaps it with the current buffer.
	pub fn swap_buffers(&mut self) {
		self.buffers[1 - self.current].reset();
		self.current = 1 - self.current;
	}

	/// Queries the backend for the terminal size.
	pub fn size(&self) -> Result<Size, B::Error> {
		self.backend.size()
	}
}

/// Computes viewport area for inline viewports.
pub(super) fn compute_inline_size<B: Backend>(
	backend: &mut B,
	height: u16,
	size: Size,
	offset_in_previous_viewport: u16,
) -> Result<(Rect, Position), B::Error> {
	let pos = backend.get_cursor_position()?;
	let mut row = pos.y;

	let max_height = size.height.min(height);

	let lines_after_cursor = height.saturating_sub(offset_in_previous_viewport).saturating_sub(1);

	backend.append_lines(lines_after_cursor)?;

	let available_lines = size.height.saturating_sub(row).saturating_sub(1);
	let missing_lines = lines_after_cursor.saturating_sub(available_lines);
	if missing_lines > 0 {
		row = row.saturating_sub(missing_lines);
	}
	row = row.saturating_sub(offset_in_previous_viewport);

	Ok((
		Rect {
			x: 0,
			y: row,
			width: size.width,
			height: max_height,
		},
		pos,
	))
}
