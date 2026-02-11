use std::collections::BTreeMap;
use std::ops::Range as StdRange;

use chrono::Local;
use termina::event::{KeyCode, KeyEvent, Modifiers};
use xeno_primitives::range::CharIdx;
use xeno_primitives::transaction::{Bias, Change};
use xeno_primitives::{EditOrigin, Mode, Range, Selection, Transaction, UndoPolicy};

#[cfg(feature = "lsp")]
use super::RenderedSnippet;
use super::vars::EditorSnippetResolver;
use super::{TransformSource, parse_snippet_template};
use crate::buffer::ViewId;
use crate::impls::Editor;

#[derive(Clone, Default)]
pub struct SnippetSessionState {
	pub session: Option<SnippetSession>,
}

#[derive(Clone, Debug)]
pub struct SnippetSession {
	pub buffer_id: ViewId,
	pub tabstops: BTreeMap<u32, Vec<StdRange<CharIdx>>>,
	pub choices: BTreeMap<u32, Vec<String>>,
	pub choice_idx: BTreeMap<u32, usize>,
	transforms: Vec<TransformBinding>,
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
	#[cfg(feature = "lsp")]
	pub(crate) fn begin_snippet_session(&mut self, buffer_id: ViewId, base_start: CharIdx, rendered: &RenderedSnippet) -> bool {
		let Some(session) = SnippetSession::from_rendered(buffer_id, base_start, rendered) else {
			return false;
		};

		let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
		state.session = Some(session);
		self.apply_active_snippet_selection()
	}

	pub(crate) fn cancel_snippet_session(&mut self) {
		self.overlays_mut().get_or_default::<SnippetSessionState>().session = None;
	}

	pub(crate) fn handle_snippet_session_key(&mut self, key: &KeyEvent) -> bool {
		if self.buffer().mode() != Mode::Insert {
			return false;
		}

		if matches!(key.code, KeyCode::Escape) {
			self.cancel_snippet_session();
			return false;
		}
		if matches!(key.code, KeyCode::Char('n')) && key.modifiers.contains(Modifiers::CONTROL) {
			return self.snippet_cycle_choice(1);
		}
		if matches!(key.code, KeyCode::Char('p')) && key.modifiers.contains(Modifiers::CONTROL) {
			return self.snippet_cycle_choice(-1);
		}

		let direction = match key.code {
			KeyCode::Tab => 1,
			KeyCode::BackTab => -1,
			KeyCode::Backspace => return self.snippet_backspace(),
			_ => return false,
		};
		let focused = self.focused_view();
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

	pub(crate) fn snippet_insert_text(&mut self, text: &str) -> bool {
		if text.is_empty() {
			return false;
		}

		let focused = self.focused_view();
		let Some((active_ranges, active_mode)) = self
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.filter(|session| session.buffer_id == focused)
			.map(|session| (session.active_ranges(), session.active_mode))
		else {
			return false;
		};

		if active_ranges.is_empty() {
			return false;
		}
		if active_mode == ActiveMode::Replace && !active_ranges.iter().any(|range| range.start < range.end) {
			return false;
		}

		let points: Vec<CharIdx> = match active_mode {
			ActiveMode::Replace => {
				if !active_ranges.iter().any(|range| range.start < range.end) {
					return false;
				}
				tx_points_for_replace(&active_ranges)
			}
			ActiveMode::Insert => tx_points_for_insert(&active_ranges),
		};

		let tx = self.buffer().with_doc(|doc| {
			let mut changes: Vec<Change> = active_ranges
				.iter()
				.map(|range| Change {
					start: if active_mode == ActiveMode::Insert { range.end } else { range.start },
					end: range.end,
					replacement: Some(text.to_string()),
				})
				.collect();
			changes.sort_by_key(|change| (change.start, change.end));
			Transaction::change(doc.content().slice(..), changes)
		});

		let mapped_points: Vec<CharIdx> = points.into_iter().map(|point| tx.changes().map_pos(point, Bias::Right)).collect();
		let Some(new_selection) = selection_from_points(mapped_points) else {
			return false;
		};

		let applied = self.apply_edit(
			focused,
			&tx,
			Some(new_selection),
			UndoPolicy::MergeWithCurrentGroup,
			EditOrigin::Internal("insert"),
		);

		if applied
			&& active_mode == ActiveMode::Replace
			&& let Some(session) = self
				.overlays_mut()
				.get_or_default::<SnippetSessionState>()
				.session
				.as_mut()
				.filter(|session| session.buffer_id == focused)
		{
			session.active_mode = ActiveMode::Insert;
		}

		applied
	}

	fn snippet_backspace(&mut self) -> bool {
		let focused = self.focused_view();
		let Some((active_ranges, active_mode)) = self
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.filter(|session| session.buffer_id == focused)
			.map(|session| (session.active_ranges(), session.active_mode))
		else {
			return false;
		};

		if active_ranges.is_empty() {
			return true;
		}

		let (changes, points): (Vec<Change>, Vec<CharIdx>) = match active_mode {
			ActiveMode::Replace => {
				let changes: Vec<Change> = active_ranges
					.iter()
					.filter(|range| range.start < range.end)
					.map(|range| Change {
						start: range.start,
						end: range.end,
						replacement: None,
					})
					.collect();
				let points: Vec<CharIdx> = active_ranges.iter().map(|range| range.start).collect();
				(changes, points)
			}
			ActiveMode::Insert => {
				let changes: Vec<Change> = active_ranges
					.iter()
					.filter(|range| range.end > range.start)
					.map(|range| Change {
						start: range.end.saturating_sub(1),
						end: range.end,
						replacement: None,
					})
					.collect();
				let points: Vec<CharIdx> = active_ranges.iter().map(|range| range.end).collect();
				(changes, points)
			}
		};

		if changes.is_empty() {
			if active_mode == ActiveMode::Replace
				&& let Some(session) = self
					.overlays_mut()
					.get_or_default::<SnippetSessionState>()
					.session
					.as_mut()
					.filter(|session| session.buffer_id == focused)
			{
				session.active_mode = ActiveMode::Insert;
			}
			return true;
		}

		let tx = self.buffer().with_doc(|doc| {
			let mut sorted = changes;
			sorted.sort_by_key(|change| (change.start, change.end));
			Transaction::change(doc.content().slice(..), sorted)
		});

		let mapped_points: Vec<CharIdx> = points.into_iter().map(|point| tx.changes().map_pos(point, Bias::Left)).collect();
		let Some(new_selection) = selection_from_points(mapped_points) else {
			return true;
		};

		let applied = self.apply_edit(
			focused,
			&tx,
			Some(new_selection),
			UndoPolicy::MergeWithCurrentGroup,
			EditOrigin::Internal("delete"),
		);

		if applied
			&& active_mode == ActiveMode::Replace
			&& let Some(session) = self
				.overlays_mut()
				.get_or_default::<SnippetSessionState>()
				.session
				.as_mut()
				.filter(|session| session.buffer_id == focused)
		{
			session.active_mode = ActiveMode::Insert;
		}

		true
	}

	fn snippet_cycle_choice(&mut self, direction: isize) -> bool {
		let focused = self.focused_view();
		let (active_ranges, replacement) = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_mut() else {
				return false;
			};
			if session.buffer_id != focused {
				state.session = None;
				return false;
			}

			let Some(index) = session.active_tabstop() else {
				return false;
			};
			let Some(options) = session.choices.get(&index) else {
				return false;
			};
			if options.is_empty() {
				return false;
			}

			let current = *session.choice_idx.get(&index).unwrap_or(&0usize);
			let len = options.len() as isize;
			let next = (current as isize + direction).rem_euclid(len) as usize;
			session.choice_idx.insert(index, next);
			(session.active_ranges(), options[next].clone())
		};

		if active_ranges.is_empty() {
			return false;
		}

		let tx = self.buffer().with_doc(|doc| {
			let mut changes: Vec<Change> = active_ranges
				.iter()
				.map(|range| Change {
					start: range.start,
					end: range.end,
					replacement: Some(replacement.clone()),
				})
				.collect();
			changes.sort_by_key(|change| (change.start, change.end));
			Transaction::change(doc.content().slice(..), changes)
		});

		let replacement_len = replacement.chars().count();
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
			EditOrigin::Internal("snippet.choice"),
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

		self.apply_edit(focused, &tx, None, UndoPolicy::MergeWithCurrentGroup, EditOrigin::Internal("snippet.transform"))
	}

	pub(crate) fn on_snippet_session_transaction(&mut self, buffer_id: ViewId, tx: &Transaction) {
		let remapped = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_mut() else {
				return;
			};
			if session.buffer_id != buffer_id {
				return;
			}
			session.remap_through(tx)
		};

		if !remapped {
			self.cancel_snippet_session();
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

fn tabstop_order(tabstops: &BTreeMap<u32, Vec<StdRange<CharIdx>>>) -> Vec<u32> {
	let mut order: Vec<u32> = tabstops.keys().copied().filter(|idx| *idx > 0).collect();
	if tabstops.contains_key(&0) {
		order.push(0);
	}
	order
}

fn active_mode_for_tabstop(tabstops: &BTreeMap<u32, Vec<StdRange<CharIdx>>>, index: u32) -> ActiveMode {
	if tabstops.get(&index).is_some_and(|ranges| ranges.iter().any(|range| range.start < range.end)) {
		ActiveMode::Replace
	} else {
		ActiveMode::Insert
	}
}

fn tx_points_for_replace(ranges: &[StdRange<CharIdx>]) -> Vec<CharIdx> {
	ranges.iter().map(|range| range.end).collect()
}

fn tx_points_for_insert(ranges: &[StdRange<CharIdx>]) -> Vec<CharIdx> {
	ranges.iter().map(|range| range.end).collect()
}

fn compute_span(tabstops: &BTreeMap<u32, Vec<StdRange<CharIdx>>>) -> Option<StdRange<CharIdx>> {
	let mut min_start: Option<CharIdx> = None;
	let mut max_end: Option<CharIdx> = None;

	for ranges in tabstops.values() {
		for range in ranges {
			min_start = Some(min_start.map_or(range.start, |current| current.min(range.start)));
			max_end = Some(max_end.map_or(range.end, |current| current.max(range.end)));
		}
	}

	Some(min_start?..max_end?)
}

fn has_overlapping_ranges(ranges: &[StdRange<CharIdx>]) -> bool {
	ranges.windows(2).any(|pair| pair[0].end > pair[1].start)
}

fn has_overlapping_changes(changes: &[Change]) -> bool {
	changes.windows(2).any(|pair| pair[1].start < pair[0].end)
}

fn primary_relative_range(ranges: &[StdRange<usize>]) -> Option<StdRange<usize>> {
	ranges.iter().min_by_key(|range| (range.start, range.end)).cloned()
}

fn normalize_ranges(mut ranges: Vec<StdRange<CharIdx>>) -> Vec<StdRange<CharIdx>> {
	ranges.sort_by_key(|range| (range.start, range.end));
	let mut out: Vec<StdRange<CharIdx>> = Vec::with_capacity(ranges.len());

	for range in ranges {
		if let Some(last) = out.last_mut() {
			if range.start == last.start && range.end == last.end {
				continue;
			}
			if range.start < last.end {
				last.end = last.end.max(range.end);
				continue;
			}
		}
		out.push(range);
	}

	out
}

fn to_selection_range(range: StdRange<CharIdx>) -> Range {
	if range.start == range.end {
		Range::point(range.start)
	} else {
		Range::from_exclusive(range.start, range.end)
	}
}

fn selection_from_points(points: Vec<CharIdx>) -> Option<Selection> {
	let mut points = points;
	points.sort_unstable();
	points.dedup();
	let primary = points.first().copied()?;
	Some(Selection::new(Range::point(primary), points.into_iter().skip(1).map(Range::point)))
}

#[cfg(test)]
mod tests {
	use termina::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, Modifiers};

	use super::*;
	use crate::impls::Editor;

	#[test]
	fn order_places_zero_last() {
		let mut tabstops = BTreeMap::new();
		#[allow(clippy::single_range_in_vec_init, reason = "these are single-element Vecs, not range expansions")]
		{
			tabstops.insert(0, vec![9..9]);
			tabstops.insert(3, vec![7..8]);
			tabstops.insert(1, vec![3..4]);
		}

		assert_eq!(tabstop_order(&tabstops), vec![1, 3, 0]);
	}

	#[test]
	fn normalize_ranges_sorts_and_merges() {
		let ranges = vec![5..8, 1..3, 3..6, 10..11];
		assert_eq!(normalize_ranges(ranges), vec![1..3, 3..8, 10..11]);
	}

	#[test]
	fn normalize_ranges_does_not_merge_adjacent() {
		let ranges = vec![0..1, 1..2];
		assert_eq!(normalize_ranges(ranges), vec![0..1, 1..2]);
	}

	#[test]
	fn normalize_ranges_dedups_points() {
		let ranges = vec![5..5, 5..5];
		assert_eq!(normalize_ranges(ranges), vec![5..5]);
	}

	fn key_tab() -> KeyEvent {
		KeyEvent {
			code: KeyCode::Tab,
			modifiers: Modifiers::NONE,
			kind: KeyEventKind::Press,
			state: KeyEventState::NONE,
		}
	}

	fn key_char(c: char) -> KeyEvent {
		KeyEvent {
			code: KeyCode::Char(c),
			modifiers: Modifiers::NONE,
			kind: KeyEventKind::Press,
			state: KeyEventState::NONE,
		}
	}

	fn key_ctrl(c: char) -> KeyEvent {
		KeyEvent {
			code: KeyCode::Char(c),
			modifiers: Modifiers::CONTROL,
			kind: KeyEventKind::Press,
			state: KeyEventState::NONE,
		}
	}

	fn buffer_text(editor: &Editor) -> String {
		editor.buffer().with_doc(|doc| doc.content().to_string())
	}

	fn primary_text(editor: &Editor) -> String {
		let range = editor.buffer().selection.primary();
		editor.buffer().with_doc(|doc| {
			let (from, to) = range.extent_clamped(doc.content().len_chars());
			doc.content().slice(from..to).to_string()
		})
	}

	fn set_multicursor_points(editor: &mut Editor, points: &[CharIdx]) {
		assert!(!points.is_empty(), "points must be non-empty");
		let primary = Range::point(points[0]);
		let others = points.iter().skip(1).copied().map(Range::point);
		let selection = Selection::new(primary, others);
		editor.buffer_mut().set_cursor_and_selection(points[0], selection);
	}

	fn set_multicursor_ranges(editor: &mut Editor, ranges: &[(CharIdx, CharIdx)]) {
		assert!(!ranges.is_empty(), "ranges must be non-empty");
		let primary = Range::from_exclusive(ranges[0].0, ranges[0].1);
		let others = ranges.iter().skip(1).map(|(start, end)| Range::from_exclusive(*start, *end));
		let selection = Selection::new(primary, others);
		editor.buffer_mut().set_cursor_and_selection(ranges[0].1, selection);
	}

	#[tokio::test]
	async fn insert_snippet_body_starts_session_and_selects_first_placeholder() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);

		assert!(editor.insert_snippet_body("a ${1:x} b ${2:y} c $0"));
		assert_eq!(buffer_text(&editor), "a x b y c ");
		assert_eq!(primary_text(&editor), "x");
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_some()
		);
	}

	#[tokio::test]
	async fn insert_snippet_body_allows_multichar_and_tab_flow() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);

		assert!(editor.insert_snippet_body("${1:x} ${2:y} $0"));
		assert_eq!(buffer_text(&editor), "x y ");
		assert_eq!(primary_text(&editor), "x");

		let _ = editor.handle_key(key_char('Q')).await;
		assert_eq!(buffer_text(&editor), "Q y ");
		let _ = editor.handle_key(key_char('W')).await;
		assert_eq!(buffer_text(&editor), "QW y ");

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert_eq!(primary_text(&editor), "y");

		let _ = editor.handle_key(key_char('Z')).await;
		assert_eq!(buffer_text(&editor), "QW Z ");

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert_eq!(primary_text(&editor), "");

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_none()
		);
	}

	#[tokio::test]
	async fn insert_snippet_body_adjacent_mirrors_do_not_merge() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);

		assert!(editor.insert_snippet_body("${1:a}${1:b}"));
		assert_eq!(buffer_text(&editor), "ab");
		assert_eq!(editor.buffer().selection.len(), 2);
		assert_eq!(primary_text(&editor), "a");

		let _ = editor.handle_key(key_char('X')).await;
		assert_eq!(buffer_text(&editor), "XX");
	}

	#[tokio::test]
	async fn insert_snippet_body_choice_cycles() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);

		assert!(editor.insert_snippet_body("${1|a,b,c|} $0"));
		assert_eq!(buffer_text(&editor), "a ");
		assert_eq!(primary_text(&editor), "a");

		assert!(editor.handle_snippet_session_key(&key_ctrl('n')));
		assert_eq!(buffer_text(&editor), "b ");
		assert_eq!(editor.buffer().selection.primary().head, 1);

		assert!(editor.handle_snippet_session_key(&key_ctrl('p')));
		assert_eq!(buffer_text(&editor), "a ");
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_some()
		);
	}

	#[tokio::test]
	async fn insert_snippet_body_choice_cycles_mirrors() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);

		assert!(editor.insert_snippet_body("${1|a,b|} ${1|a,b|} $0"));
		assert_eq!(buffer_text(&editor), "a a ");
		assert_eq!(editor.buffer().selection.len(), 2);

		assert!(editor.handle_snippet_session_key(&key_ctrl('n')));
		assert_eq!(buffer_text(&editor), "b b ");
	}

	#[tokio::test]
	async fn insert_snippet_body_choice_cycles_with_multicursor() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		editor.buffer_mut().reset_content("1\n2\n");
		set_multicursor_points(&mut editor, &[0, 2]);

		assert!(editor.insert_snippet_body("${1|x,y|} $0"));
		assert_eq!(buffer_text(&editor), "x 1\nx 2\n");
		assert_eq!(editor.buffer().selection.len(), 2);

		assert!(editor.handle_snippet_session_key(&key_ctrl('n')));
		assert_eq!(buffer_text(&editor), "y 1\ny 2\n");
	}

	#[tokio::test]
	async fn snippet_command_named_lookup_inserts_and_starts_session() {
		use crate::types::InvocationResult;

		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);

		let result = editor.invoke_command("snippet", vec!["@fori".to_string()]).await;
		assert!(matches!(result, InvocationResult::Ok));
		assert_eq!(buffer_text(&editor), "for i in 0..n {\n\t\n}");
		assert_eq!(primary_text(&editor), "i");
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_some()
		);
	}

	#[tokio::test]
	async fn insert_snippet_body_multicursor_points_starts_one_session() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		editor.buffer_mut().reset_content("a\nb\n");
		set_multicursor_points(&mut editor, &[0, 2]);

		assert!(editor.insert_snippet_body("${1:x} $0"));
		assert_eq!(buffer_text(&editor), "x a\nx b\n");
		assert_eq!(primary_text(&editor), "x");
		assert_eq!(editor.buffer().selection.len(), 2);
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_some()
		);

		let _ = editor.handle_key(key_char('Q')).await;
		assert_eq!(buffer_text(&editor), "Q a\nQ b\n");
		let _ = editor.handle_key(key_char('W')).await;
		assert_eq!(buffer_text(&editor), "QW a\nQW b\n");

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert_eq!(editor.buffer().selection.len(), 2);
		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_none()
		);
	}

	#[tokio::test]
	async fn insert_snippet_body_multicursor_no_tabstops_sets_points_and_no_session() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		editor.buffer_mut().reset_content("a\nb\n");
		set_multicursor_points(&mut editor, &[0, 2]);

		assert!(editor.insert_snippet_body("hello"));
		assert_eq!(buffer_text(&editor), "helloa\nhellob\n");
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_none()
		);
		assert_eq!(editor.buffer().selection.len(), 2);
		let points: Vec<CharIdx> = editor.buffer().selection.iter().map(|range| range.head).collect();
		assert_eq!(points, vec![5, 12]);
	}

	#[tokio::test]
	async fn insert_snippet_body_selection_variable_uses_primary_selection() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		editor.buffer_mut().reset_content("hello world");
		editor
			.buffer_mut()
			.set_cursor_and_selection(5, Selection::new(Range::from_exclusive(0, 5), std::iter::empty()));

		assert!(editor.insert_snippet_body("$SELECTION"));
		assert_eq!(buffer_text(&editor), "hello world");
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_none()
		);
	}

	#[tokio::test]
	async fn insert_snippet_body_tm_selected_text_alias_uses_primary_selection() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		editor.buffer_mut().reset_content("hello world");
		editor
			.buffer_mut()
			.set_cursor_and_selection(5, Selection::new(Range::from_exclusive(0, 5), std::iter::empty()));

		assert!(editor.insert_snippet_body("$TM_SELECTED_TEXT"));
		assert_eq!(buffer_text(&editor), "hello world");
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_none()
		);
	}

	#[tokio::test]
	async fn insert_snippet_body_malformed_transform_remains_literal_and_keeps_session() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);

		assert!(editor.insert_snippet_body("a ${1:x} ${1/(.*)/$1"));
		assert_eq!(buffer_text(&editor), "a x ${1/(.*)/$1");
		assert_eq!(primary_text(&editor), "x");
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_some()
		);

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_none()
		);
	}

	#[tokio::test]
	async fn insert_snippet_body_selection_variable_expands_per_selection() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		editor.buffer_mut().reset_content("hello world");
		set_multicursor_ranges(&mut editor, &[(0, 5), (6, 11)]);

		assert!(editor.insert_snippet_body("$SELECTION"));
		assert_eq!(buffer_text(&editor), "hello world");
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_none()
		);
		let points: Vec<CharIdx> = editor.buffer().selection.iter().map(|range| range.head).collect();
		assert_eq!(points, vec![5, 11]);
	}

	#[tokio::test]
	async fn insert_snippet_body_selection_variable_expands_per_selection_with_tabstop() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		editor.buffer_mut().reset_content("hello world");
		set_multicursor_ranges(&mut editor, &[(0, 5), (6, 11)]);

		assert!(editor.insert_snippet_body("(${SELECTION})$0"));
		assert_eq!(buffer_text(&editor), "(hello) (world)");
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_some()
		);
		let points: Vec<CharIdx> = editor.buffer().selection.iter().map(|range| range.head).collect();
		assert_eq!(points, vec![7, 15]);
	}

	#[tokio::test]
	async fn insert_snippet_body_current_second_uses_single_timestamp_across_cursors() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		editor.buffer_mut().reset_content("\n\n");
		set_multicursor_points(&mut editor, &[0, 1]);

		assert!(editor.insert_snippet_body("${CURRENT_SECOND}|${CURRENT_SECOND}"));
		let text = buffer_text(&editor);
		let lines: Vec<&str> = text.lines().collect();
		assert_eq!(lines.len(), 2);
		for line in &lines {
			let parts: Vec<&str> = line.split('|').collect();
			assert_eq!(parts.len(), 2);
			assert_eq!(parts[0], parts[1]);
			assert_eq!(parts[0].len(), 2);
			assert!(parts[0].chars().all(|ch| ch.is_ascii_digit()));
		}
		assert_eq!(lines[0], lines[1]);
	}

	#[tokio::test]
	async fn tabstop_transform_updates_on_tab() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);

		assert!(editor.insert_snippet_body("${1:foo} ${1/(.*)/$1_bar/} $0"));
		assert_eq!(buffer_text(&editor), "foo foo_bar ");
		assert_eq!(primary_text(&editor), "foo");

		let _ = editor.handle_key(key_char('x')).await;
		assert_eq!(buffer_text(&editor), "x foo_bar ");

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert_eq!(buffer_text(&editor), "x x_bar ");
		assert_eq!(primary_text(&editor), "");
	}

	#[tokio::test]
	async fn tabstop_transform_updates_per_selection_instance_on_tab() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		editor.buffer_mut().reset_content("foo\nbar\n");
		set_multicursor_ranges(&mut editor, &[(0, 3), (4, 7)]);

		assert!(editor.insert_snippet_body("${1:${SELECTION}} ${1/(.*)/$1_bar/} $0"));
		assert_eq!(buffer_text(&editor), "foo foo_bar \nbar bar_bar \n");

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert_eq!(buffer_text(&editor), "foo foo_bar \nbar bar_bar \n");
		assert_eq!(editor.buffer().selection.len(), 2);
	}

	#[cfg(feature = "lsp")]
	mod lsp_tests {
		use xeno_lsp::lsp_types::{CompletionItem, InsertTextFormat};

		use super::*;

		#[tokio::test]
		async fn lsp_snippet_session_tab_flow() {
			let mut editor = Editor::new_scratch();
			editor.set_mode(Mode::Insert);
			let buffer_id = editor.focused_view();

			let item = CompletionItem {
				label: "snippet".to_string(),
				insert_text: Some("a ${1:x} b ${2:y} c $0".to_string()),
				insert_text_format: Some(InsertTextFormat::SNIPPET),
				..Default::default()
			};

			editor.apply_completion_item(buffer_id, item).await;
			assert_eq!(buffer_text(&editor), "a x b y c ");
			assert_eq!(primary_text(&editor), "x");

			let _ = editor.handle_key(key_char('Q')).await;
			assert_eq!(buffer_text(&editor), "a Q b y c ");
			let _ = editor.handle_key(key_char('W')).await;
			assert_eq!(buffer_text(&editor), "a QW b y c ");

			assert!(editor.handle_snippet_session_key(&key_tab()));
			assert_eq!(primary_text(&editor), "y");

			let _ = editor.handle_key(key_char('Z')).await;
			assert_eq!(buffer_text(&editor), "a QW b Z c ");

			assert!(editor.handle_snippet_session_key(&key_tab()));
			assert_eq!(primary_text(&editor), "");

			assert!(editor.handle_snippet_session_key(&key_tab()));
			assert!(
				editor
					.overlays()
					.get::<SnippetSessionState>()
					.and_then(|state| state.session.as_ref())
					.is_none()
			);
			assert!(!editor.handle_snippet_session_key(&key_tab()));
		}

		#[tokio::test]
		async fn lsp_snippet_mirror_uses_multiselection_edit() {
			let mut editor = Editor::new_scratch();
			editor.set_mode(Mode::Insert);
			let buffer_id = editor.focused_view();

			let item = CompletionItem {
				label: "mirror".to_string(),
				insert_text: Some("${1:x}-$1".to_string()),
				insert_text_format: Some(InsertTextFormat::SNIPPET),
				..Default::default()
			};

			editor.apply_completion_item(buffer_id, item).await;
			assert_eq!(buffer_text(&editor), "x-");
			assert_eq!(editor.buffer().selection.len(), 2);

			let _ = editor.handle_key(key_char('Q')).await;
			assert_eq!(buffer_text(&editor), "Q-Q");
			let _ = editor.handle_key(key_char('W')).await;
			assert_eq!(buffer_text(&editor), "QW-QW");
		}
	}
}
