use tome_core::{Selection, movement};

use super::Editor;

impl Editor {
	pub(crate) fn do_search_next(&mut self, add_selection: bool, extend: bool) -> bool {
		if let Some((pattern, _reverse)) = self.input.last_search() {
			match movement::find_next(self.doc.slice(..), pattern, self.cursor + 1) {
				Ok(Some(range)) => {
					self.cursor = range.head;
					if add_selection {
						self.selection.push(range);
					} else if extend {
						let anchor = self.selection.primary().anchor;
						self.selection = Selection::single(anchor, range.max());
					} else {
						self.selection = Selection::single(range.min(), range.max());
					}
				}
				Ok(None) => {
					self.show_message("Pattern not found");
				}
				Err(e) => {
					self.show_error(format!("Regex error: {}", e));
				}
			}
		} else {
			self.show_message("No search pattern");
		}
		false
	}

	pub(crate) fn do_search_prev(&mut self, add_selection: bool, extend: bool) -> bool {
		if let Some((pattern, _reverse)) = self.input.last_search() {
			match movement::find_prev(self.doc.slice(..), pattern, self.cursor) {
				Ok(Some(range)) => {
					self.cursor = range.head;
					if add_selection {
						self.selection.push(range);
					} else if extend {
						let anchor = self.selection.primary().anchor;
						self.selection = Selection::single(anchor, range.min());
					} else {
						self.selection = Selection::single(range.min(), range.max());
					}
				}
				Ok(None) => {
					self.show_message("Pattern not found");
				}
				Err(e) => {
					self.show_error(format!("Regex error: {}", e));
				}
			}
		} else {
			self.show_message("No search pattern");
		}
		false
	}

	pub(crate) fn do_use_selection_as_search(&mut self) -> bool {
		let primary = self.selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from < to {
			let text: String = self.doc.slice(from..to).chars().collect();
			let pattern = movement::escape_pattern(&text);
			self.input.set_last_search(pattern.clone(), false);
			self.show_message(format!("Search: {}", text));
			match movement::find_next(self.doc.slice(..), &pattern, to) {
				Ok(Some(range)) => {
					self.selection = Selection::single(range.min(), range.max());
				}
				Ok(None) => {
					self.show_message("No more matches");
				}
				Err(e) => {
					self.show_error(format!("Regex error: {}", e));
				}
			}
		} else {
			self.show_message("No selection");
		}
		false
	}

	pub(crate) fn select_regex(&mut self, pattern: &str) -> bool {
		let primary = self.selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from >= to {
			self.show_message("No selection to search in");
			return false;
		}

		match movement::find_all_matches(self.doc.slice(from..to), pattern) {
			Ok(matches) if !matches.is_empty() => {
				let new_ranges: Vec<tome_core::Range> = matches
					.into_iter()
					.map(|r| tome_core::Range::new(from + r.min(), from + r.max()))
					.collect();
				self.selection = Selection::from_vec(new_ranges, 0);
				self.show_message(format!("{} matches", self.selection.len()));
			}
			Ok(_) => {
				self.show_message("No matches found");
			}
			Err(e) => {
				self.show_error(format!("Regex error: {}", e));
			}
		}
		false
	}

	pub(crate) fn split_regex(&mut self, pattern: &str) -> bool {
		let primary = self.selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from >= to {
			self.show_message("No selection to split");
			return false;
		}

		match movement::find_all_matches(self.doc.slice(from..to), pattern) {
			Ok(matches) if !matches.is_empty() => {
				let mut new_ranges: Vec<tome_core::Range> = Vec::new();
				let mut last_end = from;
				for m in matches {
					let match_start = from + m.min();
					if match_start > last_end {
						new_ranges.push(tome_core::Range::new(last_end, match_start));
					}
					last_end = from + m.max();
				}
				if last_end < to {
					new_ranges.push(tome_core::Range::new(last_end, to));
				}
				if !new_ranges.is_empty() {
					self.selection = Selection::from_vec(new_ranges, 0);
					self.show_message(format!("{} splits", self.selection.len()));
				} else {
					self.show_message("Split produced no ranges");
				}
			}
			Ok(_) => {
				self.show_message("No matches found to split on");
			}
			Err(e) => {
				self.show_error(format!("Regex error: {}", e));
			}
		}
		false
	}

	pub(crate) fn do_split_lines(&mut self) -> bool {
		let primary = self.selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from >= to {
			self.show_message("No selection to split");
			return false;
		}

		let start_line = self.doc.char_to_line(from);
		let end_line = self.doc.char_to_line(to.saturating_sub(1));

		let mut new_ranges: Vec<tome_core::Range> = Vec::new();
		for line in start_line..=end_line {
			let line_start = self.doc.line_to_char(line).max(from);
			let line_end = if line + 1 < self.doc.len_lines() {
				self.doc.line_to_char(line + 1).min(to)
			} else {
				self.doc.len_chars().min(to)
			};
			if line_start < line_end {
				new_ranges.push(tome_core::Range::new(line_start, line_end));
			}
		}

		if !new_ranges.is_empty() {
			self.selection = Selection::from_vec(new_ranges, 0);
			self.show_message(format!("{} lines", self.selection.len()));
		}
		false
	}

	pub(crate) fn keep_matching(&mut self, pattern: &str, invert: bool) -> bool {
		let mut kept_ranges: Vec<tome_core::Range> = Vec::new();
		let mut had_error = false;
		for range in self.selection.ranges() {
			let from = range.min();
			let to = range.max();
			let text: String = self.doc.slice(from..to).chars().collect();
			match movement::matches_pattern(&text, pattern) {
				Ok(matches) => {
					if matches != invert {
						kept_ranges.push(*range);
					}
				}
				Err(e) => {
					self.show_error(format!("Regex error: {}", e));
					had_error = true;
					break;
				}
			}
		}

		if had_error {
			return false;
		}

		if kept_ranges.is_empty() {
			self.show_message("No selections remain");
		} else {
			let count = kept_ranges.len();
			self.selection = Selection::from_vec(kept_ranges, 0);
			self.show_message(format!("{} selections kept", count));
		}
		false
	}
}
