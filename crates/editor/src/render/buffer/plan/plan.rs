#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
	Text { line_idx: usize, seg_idx: usize },
	PhantomTrailingNewline { line_idx: usize },
	NonTextBeyondEof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowPlan {
	pub kind: RowKind,
}

#[derive(Debug, Clone)]
pub struct ViewportPlan {
	pub rows: Vec<RowPlan>,
	pub start_line: usize,
	pub start_seg: usize,
}

impl ViewportPlan {
	pub fn new(
		start_line: usize,
		start_seg: usize,
		viewport_height: usize,
		total_lines: usize,
		has_trailing_newline: bool,
		wrap_fn: impl Fn(usize) -> usize, // line_idx -> num_segments
	) -> Self {
		let mut rows = Vec::with_capacity(viewport_height);
		let mut current_line = start_line;
		let mut current_seg = start_seg;

		while rows.len() < viewport_height && current_line < total_lines {
			let num_segs = wrap_fn(current_line).max(1);

			while rows.len() < viewport_height && current_seg < num_segs {
				rows.push(RowPlan {
					kind: RowKind::Text {
						line_idx: current_line,
						seg_idx: current_seg,
					},
				});
				current_seg += 1;
			}

			if rows.len() < viewport_height {
				// Special case: if line had trailing newline, it might have a phantom line
				// But we handle that after the main loop if it's the last line.
				current_line += 1;
				current_seg = 0;
			}
		}

		// Handle phantom trailing newline line if it fits
		if rows.len() < viewport_height && has_trailing_newline && current_line == total_lines {
			rows.push(RowPlan {
				kind: RowKind::PhantomTrailingNewline {
					line_idx: total_lines - 1,
				},
			});
		}

		// Fill remaining with NonText
		while rows.len() < viewport_height {
			rows.push(RowPlan {
				kind: RowKind::NonTextBeyondEof,
			});
		}

		Self {
			rows,
			start_line,
			start_seg,
		}
	}
}
