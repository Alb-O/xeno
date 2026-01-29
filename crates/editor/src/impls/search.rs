use xeno_primitives::Selection;
use xeno_registry::notifications::keys;

use super::Editor;
use crate::movement;

impl Editor {
	/// Applies a search hit by updating the cursor and selection.
	///
	/// Consistent with Vim, the cursor is placed at the start of the match.
	///
	/// # Arguments
	///
	/// * `hit` - The range of the matched text.
	/// * `add_selection` - Whether to add the hit to existing selections.
	/// * `extend` - Whether to extend the primary selection to the match start.
	fn apply_search_hit(
		&mut self,
		hit: xeno_primitives::range::Range,
		add_selection: bool,
		extend: bool,
	) {
		let start = hit.min();
		let end = hit.max();

		self.buffer_mut().set_cursor(start);

		if add_selection {
			self.buffer_mut()
				.selection
				.push(xeno_primitives::range::Range::new(start, end));
			return;
		}

		if extend {
			let anchor = self.buffer().selection.primary().anchor;
			self.buffer_mut()
				.set_selection(Selection::single(anchor, start));
		} else {
			self.buffer_mut()
				.set_selection(Selection::single(start, end));
		}
	}

	/// Searches forward for the current pattern.
	pub(crate) fn do_search_next(&mut self, add_selection: bool, extend: bool) -> bool {
		let search_info = self
			.buffer()
			.input
			.last_search()
			.map(|(p, r)| (p.to_string(), r));
		if let Some((pattern, _reverse)) = search_info {
			let cursor_pos = self.buffer().cursor;
			let from = cursor_pos.saturating_add(1);

			let search_result = self
				.buffer()
				.with_doc(|doc| movement::find_next(doc.content().slice(..), &pattern, from));
			match search_result {
				Ok(Some(range)) => {
					self.apply_search_hit(range, add_selection, extend);
				}
				Ok(None) => {
					self.notify(keys::PATTERN_NOT_FOUND);
				}
				Err(e) => {
					self.notify(keys::regex_error(&e.to_string()));
				}
			}
		} else {
			self.notify(keys::NO_SEARCH_PATTERN);
		}
		false
	}

	/// Searches backward for the current pattern.
	pub(crate) fn do_search_prev(&mut self, add_selection: bool, extend: bool) -> bool {
		let search_info = self
			.buffer()
			.input
			.last_search()
			.map(|(p, r)| (p.to_string(), r));
		if let Some((pattern, _reverse)) = search_info {
			let cursor_pos = self.buffer().cursor;
			let from = cursor_pos.saturating_sub(1);

			let search_result = self
				.buffer()
				.with_doc(|doc| movement::find_prev(doc.content().slice(..), &pattern, from));
			match search_result {
				Ok(Some(range)) => {
					self.apply_search_hit(range, add_selection, extend);
				}
				Ok(None) => {
					self.notify(keys::PATTERN_NOT_FOUND);
				}
				Err(e) => {
					self.notify(keys::regex_error(&e.to_string()));
				}
			}
		} else {
			self.notify(keys::NO_SEARCH_PATTERN);
		}
		false
	}

	/// Sets the current selection as the search pattern.
	pub(crate) fn do_use_selection_as_search(&mut self) -> bool {
		let primary = self.buffer().selection.primary();
		let from = primary.from();
		let to = primary.to();
		if from < to {
			let (text, pattern) = self.buffer().with_doc(|doc| {
				let text: String = doc.content().slice(from..to).chars().collect();
				let pattern = movement::escape_pattern(&text);
				(text, pattern)
			});
			self.buffer_mut()
				.input
				.set_last_search(pattern.clone(), false);
			self.notify(keys::search_info(&text));
			let search_result = self
				.buffer()
				.with_doc(|doc| movement::find_next(doc.content().slice(..), &pattern, to));
			match search_result {
				Ok(Some(range)) => {
					self.buffer_mut()
						.set_selection(Selection::single(range.min(), range.max()));
				}
				Ok(None) => {
					self.notify(keys::NO_MORE_MATCHES);
				}
				Err(e) => {
					self.notify(keys::regex_error(&e.to_string()));
				}
			}
		} else {
			self.notify(keys::NO_SELECTION);
		}
		false
	}

	/// Selects all regex matches within the current selection.
	#[allow(dead_code, reason = "regex selection will be re-enabled via picker UI")]
	pub(crate) fn select_regex(&mut self, pattern: &str) -> bool {
		let primary = self.buffer().selection.primary();
		let from = primary.from();
		let to = primary.to();
		if from >= to {
			self.notify(keys::NO_SELECTION_TO_SEARCH);
			return false;
		}

		let search_result = self
			.buffer()
			.with_doc(|doc| movement::find_all_matches(doc.content().slice(from..to), pattern));
		match search_result {
			Ok(matches) if !matches.is_empty() => {
				let new_ranges: Vec<xeno_primitives::range::Range> = matches
					.into_iter()
					.map(|r| xeno_primitives::range::Range::new(from + r.min(), from + r.max()))
					.collect();
				let count = new_ranges.len();
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, 0));
				self.notify(keys::matches_count(count));
			}
			Ok(_) => {
				self.notify(keys::NO_MATCHES_FOUND);
			}
			Err(e) => {
				self.notify(keys::regex_error(&e.to_string()));
			}
		}
		false
	}

	/// Splits the selection at regex matches, keeping the non-matching parts.
	#[allow(dead_code, reason = "regex split will be re-enabled via picker UI")]
	pub(crate) fn split_regex(&mut self, pattern: &str) -> bool {
		let primary = self.buffer().selection.primary();
		let from = primary.from();
		let to = primary.to();
		if from >= to {
			self.notify(keys::NO_SELECTION_TO_SPLIT);
			return false;
		}

		let search_result = self
			.buffer()
			.with_doc(|doc| movement::find_all_matches(doc.content().slice(from..to), pattern));
		match search_result {
			Ok(matches) if !matches.is_empty() => {
				let mut new_ranges: Vec<xeno_primitives::range::Range> = Vec::new();
				let mut last_end = from;
				for m in matches {
					let match_start = from + m.min();
					if match_start > last_end {
						new_ranges.push(xeno_primitives::range::Range::from_exclusive(
							last_end,
							match_start,
						));
					}
					last_end = from + m.to();
				}
				if last_end < to {
					new_ranges.push(xeno_primitives::range::Range::from_exclusive(last_end, to));
				}
				if !new_ranges.is_empty() {
					let count = new_ranges.len();
					self.buffer_mut()
						.set_selection(Selection::from_vec(new_ranges, 0));
					self.notify(keys::splits_count(count));
				} else {
					self.notify(keys::SPLIT_NO_RANGES);
				}
			}
			Ok(_) => {
				self.notify(keys::NO_MATCHES_TO_SPLIT);
			}
			Err(e) => {
				self.notify(keys::regex_error(&e.to_string()));
			}
		}
		false
	}

	/// Keeps only selections that match (or don't match) the pattern.
	#[allow(
		dead_code,
		reason = "keep-matching filter will be re-enabled via picker UI"
	)]
	pub(crate) fn keep_matching(&mut self, pattern: &str, invert: bool) -> bool {
		let ranges_with_text: Vec<(xeno_primitives::range::Range, String)> = {
			let buffer = self.buffer();
			buffer.with_doc(|doc| {
				buffer
					.selection
					.ranges()
					.iter()
					.map(|range| {
						let from = range.from();
						let to = range.to();
						let text: String = doc.content().slice(from..to).chars().collect();
						(*range, text)
					})
					.collect()
			})
		};

		let mut kept_ranges: Vec<xeno_primitives::range::Range> = Vec::new();
		let mut had_error = false;
		for (range, text) in ranges_with_text {
			match movement::matches_pattern(&text, pattern) {
				Ok(matches) => {
					if matches != invert {
						kept_ranges.push(range);
					}
				}
				Err(e) => {
					self.notify(keys::regex_error(&e.to_string()));
					had_error = true;
					break;
				}
			}
		}

		if had_error {
			return false;
		}

		if kept_ranges.is_empty() {
			self.notify(keys::NO_SELECTIONS_REMAIN);
		} else {
			let count = kept_ranges.len();
			self.buffer_mut()
				.set_selection(Selection::from_vec(kept_ranges, 0));
			self.notify(keys::selections_kept(count));
		}
		false
	}

	/// Repeat last search. `flip=false` => same direction as last; `flip=true` => opposite.
	pub(crate) fn do_search_repeat(
		&mut self,
		flip: bool,
		add_selection: bool,
		extend: bool,
	) -> bool {
		use xeno_registry::actions::SeqDirection;
		let Some((_, reverse)) = self.buffer().input.last_search() else {
			self.notify(keys::NO_SEARCH_PATTERN);
			return false;
		};

		let dir = match (reverse, flip) {
			(false, false) => SeqDirection::Next,
			(false, true) => SeqDirection::Prev,
			(true, false) => SeqDirection::Prev,
			(true, true) => SeqDirection::Next,
		};
		match dir {
			SeqDirection::Next => self.do_search_next(add_selection, extend),
			SeqDirection::Prev => self.do_search_prev(add_selection, extend),
		}
	}
}
