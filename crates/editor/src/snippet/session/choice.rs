use xeno_primitives::range::CharIdx;
use xeno_primitives::transaction::{Bias, Change};
use xeno_primitives::{EditOrigin, Key, KeyCode, Transaction, UndoPolicy};

use super::{ActiveMode, SnippetChoiceOverlay, SnippetSessionState, selection_from_points};
use crate::impls::Editor;

impl Editor {
	pub(super) fn open_snippet_choice_overlay(&mut self) -> bool {
		let focused = self.focused_view();
		if !self.validate_snippet_session_for_view(focused) {
			return false;
		}

		let (tabstop_idx, options, selected) = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_ref() else {
				return false;
			};
			if session.buffer_id != focused {
				state.session = None;
				return false;
			}
			let Some(tabstop_idx) = session.active_tabstop() else {
				return false;
			};
			let Some(options) = session.choices.get(&tabstop_idx) else {
				return false;
			};
			if options.is_empty() {
				return false;
			}
			let selected = (*session.choice_idx.get(&tabstop_idx).unwrap_or(&0usize)).min(options.len().saturating_sub(1));
			(tabstop_idx, options.clone(), selected)
		};

		let overlay = self.overlays_mut().get_or_default::<SnippetChoiceOverlay>();
		overlay.active = true;
		overlay.buffer_id = focused;
		overlay.tabstop_idx = tabstop_idx;
		overlay.options = options;
		overlay.selected = selected;
		self.state.frame.needs_redraw = true;
		true
	}

	pub(super) fn handle_snippet_choice_overlay_key(&mut self, key: &Key) -> bool {
		let ctrl = key.modifiers.ctrl;
		let mut close_overlay = false;
		let mut commit_choice: Option<(u32, usize, String)> = None;
		{
			let overlay = self.overlays_mut().get_or_default::<SnippetChoiceOverlay>();
			if !overlay.active {
				return false;
			}
			match key.code {
				KeyCode::Up | KeyCode::BackTab | KeyCode::Char('k') => {
					if !overlay.options.is_empty() {
						overlay.selected = if overlay.selected == 0 {
							overlay.options.len().saturating_sub(1)
						} else {
							overlay.selected.saturating_sub(1)
						};
					}
				}
				KeyCode::Down | KeyCode::Tab | KeyCode::Char('j') => {
					if !overlay.options.is_empty() {
						overlay.selected = (overlay.selected + 1) % overlay.options.len();
					}
				}
				KeyCode::Char('p') if ctrl => {
					if !overlay.options.is_empty() {
						overlay.selected = if overlay.selected == 0 {
							overlay.options.len().saturating_sub(1)
						} else {
							overlay.selected.saturating_sub(1)
						};
					}
				}
				KeyCode::Char('n') if ctrl => {
					if !overlay.options.is_empty() {
						overlay.selected = (overlay.selected + 1) % overlay.options.len();
					}
				}
				KeyCode::Enter => {
					if let Some(value) = overlay.options.get(overlay.selected).cloned() {
						commit_choice = Some((overlay.tabstop_idx, overlay.selected, value));
					}
					close_overlay = true;
				}
				KeyCode::Esc => {
					close_overlay = true;
				}
				_ => {}
			}
		}

		if close_overlay {
			self.close_snippet_choice_overlay();
		}

		if let Some((tabstop_idx, selected, value)) = commit_choice {
			let focused = self.focused_view();
			if let Some(session) = self
				.overlays_mut()
				.get_or_default::<SnippetSessionState>()
				.session
				.as_mut()
				.filter(|session| session.buffer_id == focused)
			{
				session.choice_idx.insert(tabstop_idx, selected);
			}
			return self.snippet_apply_choice_value(&value);
		}

		self.state.frame.needs_redraw = true;
		true
	}

	pub(super) fn snippet_apply_choice_value(&mut self, replacement: &str) -> bool {
		let focused = self.focused_view();
		if !self.validate_snippet_session_for_view(focused) {
			return false;
		}

		let (active_idx, active_ranges) = {
			let state = self.overlays_mut().get_or_default::<SnippetSessionState>();
			let Some(session) = state.session.as_mut() else {
				return false;
			};
			if session.buffer_id != focused {
				state.session = None;
				return false;
			}
			(session.active_tabstop(), session.active_ranges())
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
					replacement: Some(replacement.to_string()),
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

		if applied && let Some(source_idx) = active_idx {
			let _ = self.apply_transforms_for_source(source_idx);
		}

		applied
	}

	pub(super) fn snippet_cycle_choice(&mut self, direction: isize) -> bool {
		let focused = self.focused_view();
		if !self.validate_snippet_session_for_view(focused) {
			return false;
		}
		let replacement = {
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
			options[next].clone()
		};
		self.close_snippet_choice_overlay();
		self.snippet_apply_choice_value(&replacement)
	}
}
