use std::future::Future;
use std::pin::Pin;

use regex::Regex;
use xeno_primitives::Selection;
use xeno_primitives::range::Range;
use xeno_registry::notifications::keys;

use crate::buffer::ViewId;
use crate::movement;
use crate::overlay::{
	CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy,
};
use crate::window::GutterSelector;

pub struct SearchOverlay {
	target: ViewId,
	reverse: bool,
	last_input: String,
	last_preview: Option<Range>,
	cached: Option<(String, Regex)>,
	last_error: Option<String>,
}

impl SearchOverlay {
	pub fn new(target: ViewId, reverse: bool) -> Self {
		Self {
			target,
			reverse,
			last_input: String::new(),
			last_preview: None,
			cached: None,
			last_error: None,
		}
	}

	fn search_preview_find(
		&self,
		ctx: &dyn OverlayContext,
		session: &OverlaySession,
		re: &Regex,
	) -> Result<Option<Range>, regex::Error> {
		const PREVIEW_WINDOW_CHARS: usize = 200_000;
		const FULL_SCAN_PREVIEW_MAX: usize = 500_000;

		let Some(buffer) = ctx.buffer(self.target) else {
			return Ok(None);
		};

		let origin_cursor = session
			.capture
			.per_view
			.get(&self.target)
			.map(|(_, c, _)| *c)
			.unwrap_or(buffer.cursor);

		buffer.with_doc(|doc| {
			let content = doc.content();
			let len = content.len_chars();

			if len <= FULL_SCAN_PREVIEW_MAX {
				let slice = content.slice(..);
				return if self.reverse {
					Ok(movement::find_prev_re(slice, re, origin_cursor))
				} else {
					Ok(movement::find_next_re(slice, re, origin_cursor + 1))
				};
			}

			if self.reverse {
				let end = origin_cursor.min(len);
				let start = end.saturating_sub(PREVIEW_WINDOW_CHARS);
				let slice = content.slice(start..end);
				let rel_cursor = end - start;
				Ok(movement::find_prev_re(slice, re, rel_cursor).map(|r| offset_range(r, start)))
			} else {
				let start = (origin_cursor + 1).min(len);
				let end = (start + PREVIEW_WINDOW_CHARS).min(len);
				let slice = content.slice(start..end);
				Ok(movement::find_next_re(slice, re, 0).map(|r| offset_range(r, start)))
			}
		})
	}
}

fn offset_range(mut r: Range, base: usize) -> Range {
	r.anchor += base;
	r.head += base;
	r
}

impl OverlayController for SearchOverlay {
	fn name(&self) -> &'static str {
		"Search"
	}

	fn ui_spec(&self, _ctx: &dyn OverlayContext) -> OverlayUiSpec {
		OverlayUiSpec {
			title: Some(if self.reverse {
				"Search (reverse)".into()
			} else {
				"Search".into()
			}),
			gutter: GutterSelector::Prompt(if self.reverse { '?' } else { '/' }),
			rect: RectPolicy::TopCenter {
				width_percent: 60,
				max_width: 80,
				min_width: 40,
				y_frac: (1, 5),
				height: 3,
			},
			style: crate::overlay::prompt_style(if self.reverse {
				"Search (reverse)"
			} else {
				"Search"
			}),
			windows: vec![],
		}
	}

	fn on_open(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) {
		session.capture_view(ctx, self.target);
	}

	fn on_input_changed(
		&mut self,
		ctx: &mut dyn OverlayContext,
		session: &mut OverlaySession,
		text: &str,
	) {
		let input = text.trim_end_matches('\n').to_string();
		if input == self.last_input {
			return;
		}
		self.last_input = input.clone();

		if input.trim().is_empty() {
			session.restore_all(ctx);
			self.last_preview = None;
			self.last_error = None;
			self.cached = None;
			ctx.request_redraw();
			return;
		}

		let is_cached = self.cached.as_ref().is_some_and(|(p, _)| p == &input);
		if !is_cached {
			match Regex::new(&input) {
				Ok(re) => {
					self.cached = Some((input.clone(), re));
				}
				Err(e) => {
					let msg = e.to_string();
					if self.last_error.as_deref() != Some(msg.as_str()) {
						self.last_error = Some(msg.clone());
						ctx.notify(keys::regex_error(&msg));
					}
					session.restore_all(ctx);
					self.last_preview = None;
					ctx.request_redraw();
					return;
				}
			}
		}

		let Some((_, re)) = &self.cached else { return };
		let found = self.search_preview_find(ctx, session, re);

		match found {
			Ok(Some(range)) => {
				if self.last_preview != Some(range) {
					session.preview_select(ctx, self.target, range);
					self.last_preview = Some(range);
					ctx.reveal_cursor_in_view(self.target);
					ctx.request_redraw();
				}
			}
			Ok(None) => {
				if self.last_preview.is_some() {
					session.restore_all(ctx);
					self.last_preview = None;
					ctx.request_redraw();
				}
			}
			Err(e) => {
				let msg = e.to_string();
				if self.last_error.as_deref() != Some(msg.as_str()) {
					self.last_error = Some(msg.clone());
					ctx.notify(keys::regex_error(&msg));
				}
			}
		}
	}

	fn on_commit<'a>(
		&'a mut self,
		ctx: &'a mut dyn OverlayContext,
		session: &'a mut OverlaySession,
	) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let input = session
			.input_text(ctx)
			.trim_end_matches('\n')
			.trim()
			.to_string();

		if input.is_empty() {
			return Box::pin(async {});
		}

		let origin_cursor = session
			.capture
			.per_view
			.get(&self.target)
			.map(|(_, c, _)| *c)
			.unwrap_or(0);

		let result = ctx.buffer(self.target).map(|b| {
			b.with_doc(|doc| {
				let text = doc.content().slice(..);
				if self.reverse {
					movement::find_prev(text, &input, origin_cursor)
				} else {
					movement::find_next(text, &input, origin_cursor + 1)
				}
			})
		});

		match result {
			Some(Err(e)) => {
				ctx.notify(keys::regex_error(&e.to_string()));
			}
			Some(Ok(Some(range))) => {
				if let Some(buffer) = ctx.buffer_mut(self.target) {
					buffer.input.set_last_search(input.clone(), self.reverse);
					let start = range.min();
					let end = range.max();
					buffer.set_cursor(start);
					buffer.set_selection(Selection::single(start, end));
				}
				ctx.reveal_cursor_in_view(self.target);
			}
			Some(Ok(None)) => {
				if let Some(buffer) = ctx.buffer_mut(self.target) {
					buffer.input.set_last_search(input.clone(), self.reverse);
				}
				ctx.notify(keys::PATTERN_NOT_FOUND.into());
			}
			None => {}
		}

		Box::pin(async {})
	}

	fn on_close(
		&mut self,
		_ctx: &mut dyn OverlayContext,
		_session: &mut OverlaySession,
		_reason: CloseReason,
	) {
	}
}
