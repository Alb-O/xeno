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

/// Trait for accessing wrap segment counts during viewport planning.
///
/// Abstracts over different wrap data sources (closures, cache buckets, etc.)
/// to allow the viewport planner to work with any implementation.
pub trait WrapAccess {
	/// Returns the number of segments for a given line.
	fn segment_count(&self, line_idx: usize) -> usize;
}

impl<F> WrapAccess for F
where
	F: Fn(usize) -> usize,
{
	fn segment_count(&self, line_idx: usize) -> usize {
		(self)(line_idx)
	}
}

/// Viewport rendering plan.
///
/// Maps visual rows to document content (text, phantom lines, or EOF markers).
#[derive(Debug, Clone)]
pub struct ViewportPlan {
	pub rows: Vec<RowPlan>,
}

impl ViewportPlan {
	/// Creates a viewport plan using a closure for wrap counts.
	pub fn new(
		start_line: usize,
		start_seg: usize,
		viewport_height: usize,
		total_lines: usize,
		has_trailing_newline: bool,
		wrap_fn: impl Fn(usize) -> usize,
	) -> Self {
		Self::new_with_wrap(
			start_line,
			start_seg,
			viewport_height,
			total_lines,
			has_trailing_newline,
			wrap_fn,
		)
	}

	/// Creates a viewport plan using a [`WrapAccess`] implementation.
	///
	/// Passes cached wrap data directly to the planner.
	pub fn new_with_wrap(
		start_line: usize,
		start_seg: usize,
		viewport_height: usize,
		total_lines: usize,
		has_trailing_newline: bool,
		wrap_access: impl WrapAccess,
	) -> Self {
		let mut rows = Vec::with_capacity(viewport_height);
		let mut current_line = start_line;
		let mut current_seg = start_seg;

		while rows.len() < viewport_height && current_line < total_lines {
			let num_segs = wrap_access.segment_count(current_line).max(1);

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
				current_line += 1;
				current_seg = 0;
			}
		}

		if rows.len() < viewport_height && has_trailing_newline && current_line == total_lines {
			rows.push(RowPlan {
				kind: RowKind::PhantomTrailingNewline {
					line_idx: total_lines - 1,
				},
			});
		}

		while rows.len() < viewport_height {
			rows.push(RowPlan {
				kind: RowKind::NonTextBeyondEof,
			});
		}

		Self { rows }
	}
}
