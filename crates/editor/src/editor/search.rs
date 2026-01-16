use xeno_primitives::Selection;
use xeno_registry_notifications::keys;

use super::Editor;
use crate::movement;

impl Editor {
	/// Searches forward for the current pattern.
	pub(crate) fn do_search_next(&mut self, add_selection: bool, extend: bool) -> bool {
		let search_info = self
			.buffer()
			.input
			.last_search()
			.map(|(p, r)| (p.to_string(), r));
		if let Some((pattern, _reverse)) = search_info {
			let cursor_pos = self.buffer().cursor;
			let search_result = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				movement::find_next(doc.content().slice(..), &pattern, cursor_pos + 1)
			};
			match search_result {
				Ok(Some(range)) => {
					self.buffer_mut().set_cursor(range.head);
					if add_selection {
						self.buffer_mut().selection.push(range);
					} else if extend {
						let anchor = self.buffer().selection.primary().anchor;
						self.buffer_mut()
							.set_selection(Selection::single(anchor, range.max()));
					} else {
						self.buffer_mut()
							.set_selection(Selection::single(range.min(), range.max()));
					}
				}
				Ok(None) => {
					self.notify(keys::pattern_not_found);
				}
				Err(e) => {
					self.notify(keys::regex_error::call(&e.to_string()));
				}
			}
		} else {
			self.notify(keys::no_search_pattern);
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
			let search_result = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				movement::find_prev(doc.content().slice(..), &pattern, cursor_pos)
			};
			match search_result {
				Ok(Some(range)) => {
					self.buffer_mut().set_cursor(range.head);
					if add_selection {
						self.buffer_mut().selection.push(range);
					} else if extend {
						let anchor = self.buffer().selection.primary().anchor;
						self.buffer_mut()
							.set_selection(Selection::single(anchor, range.min()));
					} else {
						self.buffer_mut()
							.set_selection(Selection::single(range.min(), range.max()));
					}
				}
				Ok(None) => {
					self.notify(keys::pattern_not_found);
				}
				Err(e) => {
					self.notify(keys::regex_error::call(&e.to_string()));
				}
			}
		} else {
			self.notify(keys::no_search_pattern);
		}
		false
	}

	/// Sets the current selection as the search pattern.
	pub(crate) fn do_use_selection_as_search(&mut self) -> bool {
		let primary = self.buffer().selection.primary();
		let from = primary.from();
		let to = primary.to();
		if from < to {
			let (text, pattern) = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				let text: String = doc.content().slice(from..to).chars().collect();
				let pattern = movement::escape_pattern(&text);
				(text, pattern)
			};
			self.buffer_mut()
				.input
				.set_last_search(pattern.clone(), false);
			self.notify(keys::search_info::call(&text));
			let search_result = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				movement::find_next(doc.content().slice(..), &pattern, to)
			};
			match search_result {
				Ok(Some(range)) => {
					self.buffer_mut()
						.set_selection(Selection::single(range.min(), range.max()));
				}
				Ok(None) => {
					self.notify(keys::no_more_matches);
				}
				Err(e) => {
					self.notify(keys::regex_error::call(&e.to_string()));
				}
			}
		} else {
			self.notify(keys::no_selection);
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
			self.notify(keys::no_selection_to_search);
			return false;
		}

		let search_result = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			movement::find_all_matches(doc.content().slice(from..to), pattern)
		};
		match search_result {
			Ok(matches) if !matches.is_empty() => {
				let new_ranges: Vec<xeno_primitives::range::Range> = matches
					.into_iter()
					.map(|r| xeno_primitives::range::Range::new(from + r.min(), from + r.max()))
					.collect();
				let count = new_ranges.len();
				self.buffer_mut()
					.set_selection(Selection::from_vec(new_ranges, 0));
				self.notify(keys::matches_count::call(count));
			}
			Ok(_) => {
				self.notify(keys::no_matches_found);
			}
			Err(e) => {
				self.notify(keys::regex_error::call(&e.to_string()));
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
			self.notify(keys::no_selection_to_split);
			return false;
		}

		let search_result = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			movement::find_all_matches(doc.content().slice(from..to), pattern)
		};
		match search_result {
			Ok(matches) if !matches.is_empty() => {
				let mut new_ranges: Vec<xeno_primitives::range::Range> = Vec::new();
				let mut last_end = from;
				for m in matches {
					let match_start = from + m.min();
					if match_start > last_end {
						new_ranges.push(xeno_primitives::range::Range::new(last_end, match_start));
					}
					last_end = from + m.max();
				}
				if last_end < to {
					new_ranges.push(xeno_primitives::range::Range::new(last_end, to));
				}
				if !new_ranges.is_empty() {
					let count = new_ranges.len();
					self.buffer_mut()
						.set_selection(Selection::from_vec(new_ranges, 0));
					self.notify(keys::splits_count::call(count));
				} else {
					self.notify(keys::split_no_ranges);
				}
			}
			Ok(_) => {
				self.notify(keys::no_matches_to_split);
			}
			Err(e) => {
				self.notify(keys::regex_error::call(&e.to_string()));
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
			let doc = buffer.doc();
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
					self.notify(keys::regex_error::call(&e.to_string()));
					had_error = true;
					break;
				}
			}
		}

		if had_error {
			return false;
		}

		if kept_ranges.is_empty() {
			self.notify(keys::no_selections_remain);
		} else {
			let count = kept_ranges.len();
			self.buffer_mut()
				.set_selection(Selection::from_vec(kept_ranges, 0));
			self.notify(keys::selections_kept::call(count));
		}
		false
	}
}
