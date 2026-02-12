use unicode_width::UnicodeWidthStr;
use xeno_registry::statusline::{SegmentPosition, SegmentStyle, StatuslineContext, render_position};

use crate::impls::Editor;

/// Data-only render segment for statusline presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatuslineRenderSegment {
	pub text: String,
	pub style: StatuslineRenderStyle,
}

/// Backend-neutral style intent for a statusline segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatuslineRenderStyle {
	#[default]
	Normal,
	Mode,
	Inverted,
	Dim,
	Warning,
	Error,
	Success,
}

impl From<SegmentStyle> for StatuslineRenderStyle {
	fn from(style: SegmentStyle) -> Self {
		match style {
			SegmentStyle::Normal => Self::Normal,
			SegmentStyle::Mode => Self::Mode,
			SegmentStyle::Inverted => Self::Inverted,
			SegmentStyle::Dim => Self::Dim,
			SegmentStyle::Warning => Self::Warning,
			SegmentStyle::Error => Self::Error,
			SegmentStyle::Success => Self::Success,
		}
	}
}

fn segment_width(segment: &StatuslineRenderSegment) -> usize {
	UnicodeWidthStr::width(segment.text.as_str())
}

fn overlay_label(editor: &Editor) -> Option<&'static str> {
	let kind = editor.overlay_kind()?;
	Some(match kind {
		crate::overlay::OverlayControllerKind::CommandPalette => "Cmd",
		crate::overlay::OverlayControllerKind::Search => "Search",
		crate::overlay::OverlayControllerKind::FilePicker => "FilePicker",
		crate::overlay::OverlayControllerKind::Other(other) => other,
	})
}

fn make_segment(text: String, style: SegmentStyle) -> StatuslineRenderSegment {
	StatuslineRenderSegment { text, style: style.into() }
}

/// Builds data-only statusline content with shared width/alignment policy.
pub fn render_plan(editor: &Editor) -> Vec<StatuslineRenderSegment> {
	let buffer_ids = editor.buffer_ids();
	let buffer_index = editor
		.focused_buffer_id()
		.and_then(|current_id| buffer_ids.iter().position(|&id| id == current_id))
		.unwrap_or(0)
		+ 1;
	let buffer_count = buffer_ids.len();

	let buffer = editor.buffer();
	let path_str: Option<String> = buffer.path().as_ref().and_then(|p| p.to_str().map(ToOwned::to_owned));
	let file_type_str: Option<String> = buffer.file_type();
	let modified = buffer.modified();
	let readonly = buffer.is_readonly();
	let count = buffer.input.count();
	let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
	let mode_name = editor.mode_name();
	let line = editor.cursor_line() + 1;
	let col = editor.cursor_col() + 1;

	let (sync_role_str, sync_status_str): (Option<&str>, Option<&str>) = (None, None);

	let ctx = StatuslineContext {
		mode_name,
		path: path_str.as_deref(),
		modified,
		readonly,
		line,
		col,
		count,
		total_lines,
		file_type: file_type_str.as_deref(),
		buffer_index,
		buffer_count,
		sync_role: sync_role_str,
		sync_status: sync_status_str,
	};

	let mut mode_segments = Vec::new();
	let mut body_segments = Vec::new();
	for position in [SegmentPosition::Left, SegmentPosition::Center, SegmentPosition::Right] {
		for segment in render_position(position, &ctx) {
			let target = make_segment(segment.text, segment.style);
			if matches!(target.style, StatuslineRenderStyle::Mode) {
				mode_segments.push(target);
			} else {
				body_segments.push(target);
			}
		}
	}

	let mode_width: usize = mode_segments.iter().map(segment_width).sum();

	let mut plan = Vec::new();
	let mut current_width = 0usize;
	for segment in body_segments {
		current_width += segment_width(&segment);
		plan.push(segment);
	}

	if let Some(label) = overlay_label(editor) {
		let tag = format!(" [{label}]");
		let viewport_width = editor.viewport().width.unwrap_or(0) as usize;
		let tag_width = UnicodeWidthStr::width(tag.as_str());
		if viewport_width > 0 && current_width + tag_width + mode_width <= viewport_width {
			plan.push(StatuslineRenderSegment {
				text: tag,
				style: StatuslineRenderStyle::Dim,
			});
			current_width += tag_width;
		}
	}

	let viewport_width = editor.viewport().width.unwrap_or(0) as usize;
	if viewport_width > 0 && mode_width > 0 && current_width + mode_width < viewport_width {
		plan.push(StatuslineRenderSegment {
			text: " ".repeat(viewport_width.saturating_sub(current_width + mode_width)),
			style: StatuslineRenderStyle::Normal,
		});
	}

	plan.extend(mode_segments);
	plan
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn statusline_plan_does_not_include_overlay_tag_without_modal_overlay() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(120, 30);

		let plan = render_plan(&editor);
		assert!(!plan.iter().any(|segment| segment.text == " [Cmd]"));
	}

	#[test]
	fn statusline_plan_includes_dim_command_palette_tag_when_space_allows() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(200, 40);
		assert!(editor.open_command_palette());

		let plan = render_plan(&editor);
		let tag = plan
			.iter()
			.find(|segment| segment.text == " [Cmd]")
			.expect("statusline should include command tag");
		assert_eq!(tag.style, StatuslineRenderStyle::Dim);
	}
}
