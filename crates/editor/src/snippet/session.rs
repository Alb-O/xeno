use std::collections::BTreeMap;
use std::ops::Range as StdRange;

use termina::event::{KeyCode, KeyEvent};
use xeno_primitives::range::CharIdx;
use xeno_primitives::transaction::Bias;
use xeno_primitives::transaction::Change;
use xeno_primitives::{EditOrigin, Mode, Range, Selection, Transaction, UndoPolicy};

use super::{RenderedSnippet, parse_snippet_template, render as render_snippet};
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
	pub order: Vec<u32>,
	pub active_i: usize,
	pub span: StdRange<CharIdx>,
	pub active_mode: ActiveMode,
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
	fn from_rendered(buffer_id: ViewId, base_start: CharIdx, rendered: &RenderedSnippet) -> Option<Self> {
		let mut tabstops: BTreeMap<u32, Vec<StdRange<CharIdx>>> = rendered
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

		for ranges in tabstops.values_mut() {
			*ranges = normalize_ranges(std::mem::take(ranges));
		}
		tabstops.retain(|_, ranges| !ranges.is_empty());

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
			order,
			active_i,
			span,
			active_mode,
		})
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

		let direction = match key.code {
			KeyCode::Tab => 1,
			KeyCode::BackTab => -1,
			KeyCode::Backspace => return self.snippet_backspace(),
			_ => return false,
		};
		let focused = self.focused_view();

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

		if self.buffer().selection.len() != 1 {
			self.cancel_snippet_session();
			return false;
		}

		self.cancel_snippet_session();

		let rendered = match parse_snippet_template(body) {
			Ok(template) => render_snippet(&template),
			Err(_) => {
				self.insert_text(body);
				return true;
			}
		};

		let buffer_id = self.focused_view();
		let (start, end) = {
			let buffer = self.buffer();
			let primary = buffer.selection.primary();
			if primary.is_point() {
				(buffer.cursor, buffer.cursor)
			} else {
				(primary.from(), primary.to())
			}
		};

		let tx = self.buffer().with_doc(|doc| {
			Transaction::change(
				doc.content().slice(..),
				vec![Change {
					start,
					end,
					replacement: Some(rendered.text.clone().into()),
				}],
			)
		});

		if !self.apply_edit(buffer_id, &tx, None, UndoPolicy::Record, EditOrigin::Internal("snippet")) {
			return false;
		}

		let mapped_start = tx.changes().map_pos(start, Bias::Left);
		if !rendered.tabstops.is_empty() && self.begin_snippet_session(buffer_id, mapped_start, &rendered) {
			return true;
		}

		let cursor = mapped_start.saturating_add(rendered.text.chars().count());
		if let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) {
			buffer.set_cursor_and_selection(cursor, Selection::point(cursor));
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
					end: if active_mode == ActiveMode::Insert { range.end } else { range.end },
					replacement: Some(text.to_string().into()),
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

		if applied && active_mode == ActiveMode::Replace {
			if let Some(session) = self
				.overlays_mut()
				.get_or_default::<SnippetSessionState>()
				.session
				.as_mut()
				.filter(|session| session.buffer_id == focused)
			{
				session.active_mode = ActiveMode::Insert;
			}
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
			if active_mode == ActiveMode::Replace {
				if let Some(session) = self
					.overlays_mut()
					.get_or_default::<SnippetSessionState>()
					.session
					.as_mut()
					.filter(|session| session.buffer_id == focused)
				{
					session.active_mode = ActiveMode::Insert;
				}
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

		if applied && active_mode == ActiveMode::Replace {
			if let Some(session) = self
				.overlays_mut()
				.get_or_default::<SnippetSessionState>()
				.session
				.as_mut()
				.filter(|session| session.buffer_id == focused)
			{
				session.active_mode = ActiveMode::Insert;
			}
		}

		true
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
			return;
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

fn normalize_ranges(mut ranges: Vec<StdRange<CharIdx>>) -> Vec<StdRange<CharIdx>> {
	ranges.sort_by_key(|range| (range.start, range.end));
	let mut out: Vec<StdRange<CharIdx>> = Vec::with_capacity(ranges.len());

	for range in ranges {
		if let Some(last) = out.last_mut()
			&& range.start <= last.end
		{
			last.end = last.end.max(range.end);
			continue;
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
		tabstops.insert(0, vec![9..9]);
		tabstops.insert(3, vec![7..8]);
		tabstops.insert(1, vec![3..4]);

		assert_eq!(tabstop_order(&tabstops), vec![1, 3, 0]);
	}

	#[test]
	fn normalize_ranges_sorts_and_merges() {
		let ranges = vec![5..8, 1..3, 3..6, 10..11];
		assert_eq!(normalize_ranges(ranges), vec![1..8, 10..11]);
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
