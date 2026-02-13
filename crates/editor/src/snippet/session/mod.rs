use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range as StdRange;

use chrono::Local;
use xeno_primitives::range::CharIdx;
use xeno_primitives::transaction::{Bias, Change};
use xeno_primitives::{EditOrigin, Key, KeyCode, Mode, Selection, Transaction, UndoPolicy};

mod choice;
mod helpers;

use helpers::{
	active_mode_for_tabstop, compute_span, has_overlapping_changes, has_overlapping_ranges, normalize_ranges, primary_relative_range, selection_from_points,
	tabstop_order, to_selection_range,
};

#[cfg(feature = "lsp")]
use super::RenderedSnippet;
use super::vars::EditorSnippetResolver;
use super::{TransformSource, parse_snippet_template};
use crate::Editor;
use crate::buffer::ViewId;

#[derive(Clone, Default)]
pub struct SnippetSessionState {
	pub session: Option<SnippetSession>,
}

#[derive(Clone)]
pub struct SnippetChoiceOverlay {
	pub active: bool,
	pub buffer_id: ViewId,
	pub tabstop_idx: u32,
	pub options: Vec<String>,
	pub selected: usize,
}

impl Default for SnippetChoiceOverlay {
	fn default() -> Self {
		Self {
			active: false,
			buffer_id: ViewId(0),
			tabstop_idx: 0,
			options: Vec::new(),
			selected: 0,
		}
	}
}

#[derive(Clone, Debug)]
pub struct SnippetSession {
	pub buffer_id: ViewId,
	pub tabstops: BTreeMap<u32, Vec<StdRange<CharIdx>>>,
	pub choices: BTreeMap<u32, Vec<String>>,
	pub choice_idx: BTreeMap<u32, usize>,
	transforms: Vec<TransformBinding>,
	dirty_sources: BTreeSet<u32>,
	in_transform_apply: bool,
	pub order: Vec<u32>,
	pub active_i: usize,
	pub span: StdRange<CharIdx>,
	pub active_mode: ActiveMode,
}

#[derive(Clone, Debug)]
struct TransformBinding {
	source_idx: u32,
	source_range: StdRange<CharIdx>,
	target_range: StdRange<CharIdx>,
	regex: String,
	replace: String,
	flags: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveMode {
	Replace,
	Insert,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AdvanceResult {
	Moved,
	Stayed,
	End,
}

impl SnippetSession {
	fn from_components(
		buffer_id: ViewId,
		mut tabstops: BTreeMap<u32, Vec<StdRange<CharIdx>>>,
		mut choices: BTreeMap<u32, Vec<String>>,
		mut transforms: Vec<TransformBinding>,
	) -> Option<Self> {
		for ranges in tabstops.values_mut() {
			*ranges = normalize_ranges(std::mem::take(ranges));
		}
		tabstops.retain(|_, ranges| !ranges.is_empty());
		choices.retain(|idx, options| tabstops.contains_key(idx) && !options.is_empty());
		transforms.retain(|binding| tabstops.contains_key(&binding.source_idx));
		let choice_idx: BTreeMap<u32, usize> = choices.keys().map(|idx| (*idx, 0usize)).collect();

		let order = tabstop_order(&tabstops);
		if order.is_empty() {
			return None;
		}

		let span = compute_span(&tabstops)?;
		let active_i = 0;
		let active_mode = active_mode_for_tabstop(&tabstops, order[active_i]);

		Some(Self {
			buffer_id,
			tabstops,
			choices,
			choice_idx,
			transforms,
			dirty_sources: BTreeSet::new(),
			in_transform_apply: false,
			order,
			active_i,
			span,
			active_mode,
		})
	}

	#[cfg(feature = "lsp")]
	fn from_rendered(buffer_id: ViewId, base_start: CharIdx, rendered: &RenderedSnippet) -> Option<Self> {
		let tabstops: BTreeMap<u32, Vec<StdRange<CharIdx>>> = rendered
			.tabstops
			.iter()
			.map(|(&index, ranges)| {
				let absolute = ranges
					.iter()
					.map(|range| {
						let start = base_start.saturating_add(range.start);
						let end = base_start.saturating_add(range.end);
						start..end
					})
					.collect();
				(index, absolute)
			})
			.collect();

		let primary_sources: BTreeMap<u32, StdRange<CharIdx>> = rendered
			.tabstops
			.iter()
			.filter_map(|(&idx, ranges)| {
				let rel = primary_relative_range(ranges)?;
				Some((idx, base_start.saturating_add(rel.start)..base_start.saturating_add(rel.end)))
			})
			.collect();
		let transforms: Vec<TransformBinding> = rendered
			.transforms
			.iter()
			.filter_map(|transform| {
				let TransformSource::Tabstop(source_idx) = transform.source else {
					return None;
				};
				let source_range = primary_sources.get(&source_idx)?.clone();
				let target_range = base_start.saturating_add(transform.range.start)..base_start.saturating_add(transform.range.end);
				Some(TransformBinding {
					source_idx,
					source_range,
					target_range,
					regex: transform.regex.clone(),
					replace: transform.replace.clone(),
					flags: transform.flags.clone(),
				})
			})
			.collect();

		Self::from_components(buffer_id, tabstops, rendered.choices.clone(), transforms)
	}

	fn active_tabstop(&self) -> Option<u32> {
		self.order.get(self.active_i).copied()
	}

	fn active_ranges(&self) -> Vec<StdRange<CharIdx>> {
		self.active_tabstop().and_then(|idx| self.tabstops.get(&idx).cloned()).unwrap_or_default()
	}

	fn advance(&mut self, direction: isize) -> AdvanceResult {
		if direction > 0 {
			if self.active_i + 1 >= self.order.len() {
				return AdvanceResult::End;
			}
			self.active_i += 1;
			if let Some(index) = self.active_tabstop() {
				self.active_mode = active_mode_for_tabstop(&self.tabstops, index);
			}
			return AdvanceResult::Moved;
		}
		if direction < 0 {
			if self.active_i == 0 {
				return AdvanceResult::Stayed;
			}
			self.active_i -= 1;
			if let Some(index) = self.active_tabstop() {
				self.active_mode = active_mode_for_tabstop(&self.tabstops, index);
			}
			return AdvanceResult::Moved;
		}
		AdvanceResult::Stayed
	}

	fn remap_through(&mut self, tx: &Transaction) -> bool {
		for ranges in self.tabstops.values_mut() {
			for range in ranges.iter_mut() {
				let start = tx.changes().map_pos(range.start, Bias::Left);
				let end = tx.changes().map_pos(range.end, Bias::Right);
				range.start = start;
				range.end = end.max(start);
			}
			*ranges = normalize_ranges(std::mem::take(ranges));
		}

		self.tabstops.retain(|_, ranges| !ranges.is_empty());
		for binding in &mut self.transforms {
			let source_start = tx.changes().map_pos(binding.source_range.start, Bias::Left);
			let source_end = tx.changes().map_pos(binding.source_range.end, Bias::Right);
			binding.source_range.start = source_start;
			binding.source_range.end = source_end.max(source_start);

			let target_start = tx.changes().map_pos(binding.target_range.start, Bias::Left);
			let target_end = tx.changes().map_pos(binding.target_range.end, Bias::Right);
			binding.target_range.start = target_start;
			binding.target_range.end = target_end.max(target_start);
		}
		self.transforms.retain(|binding| self.tabstops.contains_key(&binding.source_idx));
		self.order = tabstop_order(&self.tabstops);
		if self.order.is_empty() {
			return false;
		}
		if self.active_i >= self.order.len() {
			self.active_i = self.order.len().saturating_sub(1);
		}

		if let Some(span) = compute_span(&self.tabstops) {
			self.span = span;
			true
		} else {
			false
		}
	}
}

impl Editor {
	fn validate_snippet_session_for_view(&mut self, view: ViewId) -> bool {
		let (active_ranges, span) = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_ref() else {
				return false;
			};
			if session.buffer_id != view {
				state.session = None;
				return false;
			}
			if session.active_tabstop().is_none() {
				state.session = None;
				return false;
			}
			let active_ranges = normalize_ranges(session.active_ranges());
			if active_ranges.is_empty() {
				state.session = None;
				return false;
			}
			(active_ranges, session.span.clone())
		};

		let Some(buffer) = self.state.core.buffers.get_buffer(view) else {
			self.cancel_snippet_session();
			return false;
		};
		let selection_len = buffer.selection.len();
		let selection_heads: Vec<CharIdx> = buffer.selection.iter().map(|range| range.head).collect();

		if selection_len != active_ranges.len() {
			self.cancel_snippet_session();
			return false;
		}

		for head in selection_heads {
			if head < span.start || head > span.end {
				self.cancel_snippet_session();
				return false;
			}
			if !active_ranges.iter().any(|active| head >= active.start && head <= active.end) {
				self.cancel_snippet_session();
				return false;
			}
		}

		true
	}

	pub(crate) fn snippet_session_on_cursor_moved(&mut self, view: ViewId) {
		let _ = self.validate_snippet_session_for_view(view);
	}

	#[cfg(feature = "lsp")]
	pub(crate) fn begin_snippet_session(&mut self, buffer_id: ViewId, base_start: CharIdx, rendered: &RenderedSnippet) -> bool {
		let Some(session) = SnippetSession::from_rendered(buffer_id, base_start, rendered) else {
			return false;
		};

		let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
		state.session = Some(session);
		self.apply_active_snippet_selection()
	}

	fn close_snippet_choice_overlay(&mut self) {
		*self.overlays_mut().get_or_default::<SnippetChoiceOverlay>() = SnippetChoiceOverlay::default();
	}

	pub(crate) fn cancel_snippet_session(&mut self) {
		self.overlays_mut().get_or_default::<SnippetSessionState>().session = None;
		self.close_snippet_choice_overlay();
	}

	pub(crate) fn handle_snippet_session_key(&mut self, key: &Key) -> bool {
		if self.buffer().mode() != Mode::Insert {
			return false;
		}
		let focused = self.focused_view();
		if !self.validate_snippet_session_for_view(focused) {
			return false;
		}
		if self.handle_snippet_choice_overlay_key(key) {
			return true;
		}

		if matches!(key.code, KeyCode::Esc) {
			self.cancel_snippet_session();
			return false;
		}
		if matches!(key.code, KeyCode::Char(' ') | KeyCode::Space) && key.modifiers.ctrl {
			return self.open_snippet_choice_overlay();
		}
		if matches!(key.code, KeyCode::Char('n')) && key.modifiers.ctrl {
			return self.snippet_cycle_choice(1);
		}
		if matches!(key.code, KeyCode::Char('p')) && key.modifiers.ctrl {
			return self.snippet_cycle_choice(-1);
		}
		if matches!(key.code, KeyCode::Backspace) {
			return self.snippet_replace_mode_backspace();
		}

		let direction = match key.code {
			KeyCode::Tab => 1,
			KeyCode::BackTab => -1,
			_ => return false,
		};
		self.close_snippet_choice_overlay();
		let prev_idx = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_mut() else {
				return false;
			};
			if session.buffer_id != focused {
				state.session = None;
				return false;
			}
			session.active_tabstop()
		};
		if let Some(idx) = prev_idx {
			let _ = self.apply_transforms_for_source(idx);
		}

		let action = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_mut() else {
				return false;
			};
			if session.buffer_id != focused {
				state.session = None;
				return false;
			}
			session.advance(direction)
		};

		match action {
			AdvanceResult::End => {
				self.cancel_snippet_session();
				self.buffer_mut().clear_undo_group();
				self.state.frame.needs_redraw = true;
				true
			}
			AdvanceResult::Moved | AdvanceResult::Stayed => {
				let _ = self.apply_active_snippet_selection();
				true
			}
		}
	}

	pub fn insert_snippet_body(&mut self, body: &str) -> bool {
		if !self.guard_readonly() {
			return false;
		}

		self.cancel_snippet_session();
		let buffer_id = self.focused_view();

		let template = match parse_snippet_template(body) {
			Ok(template) => template,
			Err(_) => {
				self.insert_text(body);
				return true;
			}
		};

		let now = Local::now();
		let mut edit_inputs: Vec<(StdRange<CharIdx>, String)> = {
			let buffer = self.buffer();
			buffer.with_doc(|doc| {
				let doc_len = doc.content().len_chars();
				buffer
					.selection
					.iter()
					.map(|range| {
						if range.is_point() {
							(range.head..range.head, String::new())
						} else {
							let (from, to) = range.extent_clamped(doc_len);
							(from..to, doc.content().slice(from..to).to_string())
						}
					})
					.collect()
			})
		};
		edit_inputs.sort_by_key(|(range, _)| (range.start, range.end));
		let edit_ranges: Vec<StdRange<CharIdx>> = edit_inputs.iter().map(|(range, _)| range.clone()).collect();
		if has_overlapping_ranges(&edit_ranges) {
			self.cancel_snippet_session();
			return false;
		}

		let rendered_snippets: Vec<_> = edit_inputs
			.iter()
			.map(|(_, selection_text)| {
				let resolver = EditorSnippetResolver::new_for_selection(self, buffer_id, Some(selection_text.clone()), now);
				super::render_with_resolver(&template, &resolver)
			})
			.collect();

		let tx = self.buffer().with_doc(|doc| {
			Transaction::change(
				doc.content().slice(..),
				edit_inputs
					.iter()
					.zip(rendered_snippets.iter())
					.map(|((range, _), rendered)| Change {
						start: range.start,
						end: range.end,
						replacement: Some(rendered.text.clone()),
					})
					.collect::<Vec<_>>(),
			)
		});

		if !self.apply_edit(buffer_id, &tx, None, UndoPolicy::Record, EditOrigin::Internal("snippet")) {
			return false;
		}

		let mapped_starts: Vec<CharIdx> = edit_inputs.iter().map(|(range, _)| tx.changes().map_pos(range.start, Bias::Left)).collect();

		if rendered_snippets.iter().any(|rendered| !rendered.tabstops.is_empty()) {
			let mut tabstops: BTreeMap<u32, Vec<StdRange<CharIdx>>> = BTreeMap::new();
			let mut choices: BTreeMap<u32, Vec<String>> = BTreeMap::new();
			let mut transforms: Vec<TransformBinding> = Vec::new();
			for (mapped_start, rendered) in mapped_starts.iter().copied().zip(rendered_snippets.iter()) {
				let primary_sources: BTreeMap<u32, StdRange<CharIdx>> = rendered
					.tabstops
					.iter()
					.filter_map(|(&idx, ranges)| {
						let rel = primary_relative_range(ranges)?;
						Some((idx, mapped_start.saturating_add(rel.start)..mapped_start.saturating_add(rel.end)))
					})
					.collect();
				for (&index, ranges) in &rendered.tabstops {
					let entry = tabstops.entry(index).or_default();
					for range in ranges {
						let start = mapped_start.saturating_add(range.start);
						let end = mapped_start.saturating_add(range.end);
						entry.push(start..end);
					}
				}
				for (&index, options) in &rendered.choices {
					choices.entry(index).or_insert_with(|| options.clone());
				}
				for transform in &rendered.transforms {
					let TransformSource::Tabstop(source_idx) = transform.source else {
						continue;
					};
					let Some(source_range) = primary_sources.get(&source_idx).cloned() else {
						continue;
					};
					transforms.push(TransformBinding {
						source_idx,
						source_range,
						target_range: mapped_start.saturating_add(transform.range.start)..mapped_start.saturating_add(transform.range.end),
						regex: transform.regex.clone(),
						replace: transform.replace.clone(),
						flags: transform.flags.clone(),
					});
				}
			}

			if let Some(session) = SnippetSession::from_components(buffer_id, tabstops, choices, transforms) {
				self.overlays_mut().get_or_default::<SnippetSessionState>().session = Some(session);
				return self.apply_active_snippet_selection();
			}
		}

		let points: Vec<CharIdx> = mapped_starts
			.into_iter()
			.zip(rendered_snippets.iter())
			.map(|(mapped_start, rendered)| mapped_start.saturating_add(rendered.text.chars().count()))
			.collect();
		let Some(selection) = selection_from_points(points) else {
			return false;
		};
		if let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) {
			let cursor = selection.primary().head;
			buffer.set_cursor_and_selection(cursor, selection);
		}
		true
	}

	pub(crate) fn snippet_replace_mode_insert(&mut self, text: &str) -> bool {
		if text.is_empty() {
			return false;
		}

		let focused = self.focused_view();
		if !self.validate_snippet_session_for_view(focused) {
			return false;
		}
		let (active_idx, active_ranges) = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_ref() else {
				return false;
			};
			if session.buffer_id != focused {
				state.session = None;
				return false;
			}
			if session.active_mode != ActiveMode::Replace {
				return false;
			}
			(session.active_tabstop(), session.active_ranges())
		};

		if active_ranges.is_empty() || !active_ranges.iter().any(|range| range.start < range.end) {
			return false;
		}

		let tx = self.buffer().with_doc(|doc| {
			let mut changes: Vec<Change> = active_ranges
				.iter()
				.map(|range| Change {
					start: range.start,
					end: range.end,
					replacement: Some(text.to_string()),
				})
				.collect();
			changes.sort_by_key(|change| (change.start, change.end));
			Transaction::change(doc.content().slice(..), changes)
		});

		let replacement_len = text.chars().count();
		let mapped_points: Vec<CharIdx> = active_ranges
			.iter()
			.map(|range| tx.changes().map_pos(range.start, Bias::Left).saturating_add(replacement_len))
			.collect();
		let Some(new_selection) = selection_from_points(mapped_points) else {
			return false;
		};

		let applied = self.apply_edit(
			focused,
			&tx,
			Some(new_selection),
			UndoPolicy::MergeWithCurrentGroup,
			EditOrigin::Internal("snippet.replace"),
		);

		if applied
			&& let Some(session) = self
				.overlays_mut()
				.get_or_default::<SnippetSessionState>()
				.session
				.as_mut()
				.filter(|session| session.buffer_id == focused)
		{
			session.active_mode = ActiveMode::Insert;
		}

		if applied && let Some(source_idx) = active_idx {
			let _ = self.apply_transforms_for_source(source_idx);
		}

		applied
	}

	fn snippet_replace_mode_backspace(&mut self) -> bool {
		let focused = self.focused_view();
		if !self.validate_snippet_session_for_view(focused) {
			return false;
		}
		let (active_idx, active_ranges) = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_ref() else {
				return false;
			};
			if session.buffer_id != focused {
				state.session = None;
				return false;
			}
			if session.active_mode != ActiveMode::Replace {
				return false;
			}
			(session.active_tabstop(), session.active_ranges())
		};

		if active_ranges.is_empty() || !active_ranges.iter().any(|range| range.start < range.end) {
			return false;
		}

		let tx = self.buffer().with_doc(|doc| {
			let mut changes: Vec<Change> = active_ranges
				.iter()
				.filter(|range| range.start < range.end)
				.map(|range| Change {
					start: range.start,
					end: range.end,
					replacement: None,
				})
				.collect();
			changes.sort_by_key(|change| (change.start, change.end));
			Transaction::change(doc.content().slice(..), changes)
		});

		let mapped_points: Vec<CharIdx> = active_ranges.iter().map(|range| tx.changes().map_pos(range.start, Bias::Left)).collect();
		let Some(new_selection) = selection_from_points(mapped_points) else {
			return false;
		};

		let applied = self.apply_edit(
			focused,
			&tx,
			Some(new_selection),
			UndoPolicy::MergeWithCurrentGroup,
			EditOrigin::Internal("snippet.replace.delete"),
		);

		if applied
			&& let Some(session) = self
				.overlays_mut()
				.get_or_default::<SnippetSessionState>()
				.session
				.as_mut()
				.filter(|session| session.buffer_id == focused)
		{
			session.active_mode = ActiveMode::Insert;
		}

		if applied && let Some(source_idx) = active_idx {
			let _ = self.apply_transforms_for_source(source_idx);
		}

		applied
	}

	fn apply_transforms_for_source(&mut self, source_idx: u32) -> bool {
		let focused = self.focused_view();
		let bindings: Vec<TransformBinding> = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_ref() else {
				return false;
			};
			if session.buffer_id != focused {
				state.session = None;
				return false;
			}
			if session.in_transform_apply {
				return false;
			}
			session.transforms.iter().filter(|binding| binding.source_idx == source_idx).cloned().collect()
		};

		if bindings.is_empty() {
			return false;
		}

		let tx = self.buffer().with_doc(|doc| {
			let doc_len = doc.content().len_chars();
			let mut changes: Vec<Change> = bindings
				.iter()
				.map(|binding| {
					let source_start = binding.source_range.start.min(doc_len);
					let source_end = binding.source_range.end.min(doc_len).max(source_start);
					let source_text = doc.content().slice(source_start..source_end).to_string();
					let output = super::render::apply_transform(&source_text, &binding.regex, &binding.replace, &binding.flags);
					Change {
						start: binding.target_range.start,
						end: binding.target_range.end,
						replacement: Some(output),
					}
				})
				.collect();
			changes.sort_by_key(|change| (change.start, change.end));
			if has_overlapping_changes(&changes) {
				return None;
			}
			Some(Transaction::change(doc.content().slice(..), changes))
		});

		let Some(tx) = tx else {
			return false;
		};

		{
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_mut() else {
				return false;
			};
			if session.buffer_id != focused {
				state.session = None;
				return false;
			}
			session.in_transform_apply = true;
		}

		let applied = self.apply_edit(focused, &tx, None, UndoPolicy::MergeWithCurrentGroup, EditOrigin::Internal("snippet.transform"));

		if let Some(session) = self
			.overlays_mut()
			.get_or_default::<SnippetSessionState>()
			.session
			.as_mut()
			.filter(|session| session.buffer_id == focused)
		{
			session.in_transform_apply = false;
			if applied {
				session.dirty_sources.remove(&source_idx);
			}
		}

		applied
	}

	pub(crate) fn on_snippet_session_transaction(&mut self, buffer_id: ViewId, tx: &Transaction) {
		let (remapped, in_transform_apply) = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_mut() else {
				return;
			};
			if session.buffer_id != buffer_id {
				return;
			}
			let remapped = session.remap_through(tx);
			(remapped, session.in_transform_apply)
		};

		if !remapped {
			self.cancel_snippet_session();
			return;
		}
		if !self.validate_snippet_session_for_view(buffer_id) {
			return;
		}
		if in_transform_apply {
			return;
		}

		let pending_transform = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_mut() else {
				return;
			};
			if session.buffer_id != buffer_id {
				state.session = None;
				return;
			}
			let Some(active_idx) = session.active_tabstop() else {
				return;
			};
			session.dirty_sources.insert(active_idx);
			(session.active_mode == ActiveMode::Insert).then_some(active_idx)
		};

		if let Some(source_idx) = pending_transform {
			let _ = self.apply_transforms_for_source(source_idx);
		}
	}

	fn apply_active_snippet_selection(&mut self) -> bool {
		let Some((buffer_id, ranges, mode)) = self
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.map(|session| (session.buffer_id, session.active_ranges(), session.active_mode))
		else {
			return false;
		};

		let ranges = match mode {
			ActiveMode::Replace => normalize_ranges(ranges),
			ActiveMode::Insert => normalize_ranges(ranges.into_iter().map(|range| range.end..range.end).collect()),
		};
		let Some(primary) = ranges.first().cloned() else {
			self.cancel_snippet_session();
			return false;
		};

		let selection = Selection::new(to_selection_range(primary), ranges.into_iter().skip(1).map(to_selection_range));

		let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) else {
			self.cancel_snippet_session();
			return false;
		};
		let cursor = selection.primary().head;
		buffer.set_cursor_and_selection(cursor, selection);
		self.state.frame.needs_redraw = true;
		true
	}
}

#[cfg(test)]
mod tests;
