//! Viewport layout planning.
//!
//! Orchestrates the mapping between visual rows in the UI and physical lines
//! in the document, handling soft-wrapping and EOF markers.

/// Type of content for a visual row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
	/// Row contains text from a specific document line and wrap segment.
	Text {
		/// The 0-based index of the physical line.
		line_idx: usize,
		/// The 0-based index of the wrap segment within that line.
		seg_idx: usize,
	},
	/// Row is beyond the end of the document (typically rendered as '~').
	NonTextBeyondEof,
}

/// A plan for a single visual row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowPlan {
	/// The kind of content to render in this row.
	pub kind: RowKind,
}

/// Trait for accessing wrap segment counts during viewport planning.
///
/// This abstraction allows the planner to work with either cached wrap data
/// or on-the-fly calculations without knowing the underlying implementation.
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

/// A complete plan for a viewport's rows.
///
/// Maps visual rows to document content. The plan is generated once per
/// render pass to ensure layout consistency.
#[derive(Debug, Clone)]
pub struct ViewportPlan {
	/// The planned visual rows.
	pub rows: Vec<RowPlan>,
}

impl ViewportPlan {
	/// Creates a viewport plan using a closure for wrap counts.
	///
	/// Use this when wrap data is computed on-the-fly.
	pub fn new(
		start_line: usize,
		start_seg: usize,
		viewport_height: usize,
		total_lines: usize,
		wrap_fn: impl Fn(usize) -> usize,
	) -> Self {
		Self::new_with_wrap(start_line, start_seg, viewport_height, total_lines, wrap_fn)
	}

	/// Creates a viewport plan using a [`WrapAccess`] implementation.
	///
	/// Passes cached wrap data directly to the planner.
	pub fn new_with_wrap(
		start_line: usize,
		start_seg: usize,
		viewport_height: usize,
		total_lines: usize,
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

		while rows.len() < viewport_height {
			rows.push(RowPlan {
				kind: RowKind::NonTextBeyondEof,
			});
		}

		Self { rows }
	}
}
