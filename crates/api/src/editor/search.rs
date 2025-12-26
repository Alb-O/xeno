use tome_base::Selection;
use tome_stdlib::movement;

use super::Editor;

impl Editor {
	pub(crate) fn do_search_next(&mut self, add_selection: bool, extend: bool) -> bool {
		if let Some((pattern, _reverse)) = self.buffer().input.last_search() {
			match movement::find_next(
				self.buffer().doc.slice(..),
				pattern,
				self.buffer().cursor + 1,
			) {
				Ok(Some(range)) => {
					self.buffer_mut().cursor = range.head;
					if add_selection {
						self.buffer_mut().selection.push(range);
					} else if extend {
						let anchor = self.buffer().selection.primary().anchor;
						self.buffer_mut().selection = Selection::single(anchor, range.max());
					} else {
						self.buffer_mut().selection = Selection::single(range.min(), range.max());
					}
				}
				Ok(None) => {
					self.notify("warn", "Pattern not found");
				}
				Err(e) => {
					self.notify("error", format!("Regex error: {}", e));
				}
			}
		} else {
			self.notify("warn", "No search pattern");
		}
		false
	}

	pub(crate) fn do_search_prev(&mut self, add_selection: bool, extend: bool) -> bool {
		if let Some((pattern, _reverse)) = self.buffer().input.last_search() {
			match movement::find_prev(self.buffer().doc.slice(..), pattern, self.buffer().cursor) {
				Ok(Some(range)) => {
					self.buffer_mut().cursor = range.head;
					if add_selection {
						self.buffer_mut().selection.push(range);
					} else if extend {
						let anchor = self.buffer().selection.primary().anchor;
						self.buffer_mut().selection = Selection::single(anchor, range.min());
					} else {
						self.buffer_mut().selection = Selection::single(range.min(), range.max());
					}
				}
				Ok(None) => {
					self.notify("warn", "Pattern not found");
				}
				Err(e) => {
					self.notify("error", format!("Regex error: {}", e));
				}
			}
		} else {
			self.notify("warn", "No search pattern");
		}
		false
	}

	pub(crate) fn do_use_selection_as_search(&mut self) -> bool {
		let primary = self.buffer().selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from < to {
			let text: String = self.buffer().doc.slice(from..to).chars().collect();
			let pattern = movement::escape_pattern(&text);
			self.buffer_mut()
				.input
				.set_last_search(pattern.clone(), false);
			self.notify("info", format!("Search: {}", text));
			match movement::find_next(self.buffer().doc.slice(..), &pattern, to) {
				Ok(Some(range)) => {
					self.buffer_mut().selection = Selection::single(range.min(), range.max());
				}
				Ok(None) => {
					self.notify("warn", "No more matches");
				}
				Err(e) => {
					self.notify("error", format!("Regex error: {}", e));
				}
			}
		} else {
			self.notify("warn", "No selection");
		}
		false
	}

	#[allow(dead_code, reason = "regex selection will be re-enabled via picker UI")]
	pub(crate) fn select_regex(&mut self, pattern: &str) -> bool {
		let primary = self.buffer().selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from >= to {
			self.notify("warn", "No selection to search in");
			return false;
		}

		match movement::find_all_matches(self.buffer().doc.slice(from..to), pattern) {
			Ok(matches) if !matches.is_empty() => {
				let new_ranges: Vec<tome_base::range::Range> = matches
					.into_iter()
					.map(|r| tome_base::range::Range::new(from + r.min(), from + r.max()))
					.collect();
				self.buffer_mut().selection = Selection::from_vec(new_ranges, 0);
				self.notify("info", format!("{} matches", self.buffer().selection.len()));
			}
			Ok(_) => {
				self.notify("warn", "No matches found");
			}
			Err(e) => {
				self.notify("error", format!("Regex error: {}", e));
			}
		}
		false
	}

	#[allow(dead_code, reason = "regex split will be re-enabled via picker UI")]
	pub(crate) fn split_regex(&mut self, pattern: &str) -> bool {
		let primary = self.buffer().selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from >= to {
			self.notify("warn", "No selection to split");
			return false;
		}

		match movement::find_all_matches(self.buffer().doc.slice(from..to), pattern) {
			Ok(matches) if !matches.is_empty() => {
				let mut new_ranges: Vec<tome_base::range::Range> = Vec::new();
				let mut last_end = from;
				for m in matches {
					let match_start = from + m.min();
					if match_start > last_end {
						new_ranges.push(tome_base::range::Range::new(last_end, match_start));
					}
					last_end = from + m.max();
				}
				if last_end < to {
					new_ranges.push(tome_base::range::Range::new(last_end, to));
				}
				if !new_ranges.is_empty() {
					self.buffer_mut().selection = Selection::from_vec(new_ranges, 0);
					self.notify("info", format!("{} splits", self.buffer().selection.len()));
				} else {
					self.notify("warn", "Split produced no ranges");
				}
			}
			Ok(_) => {
				self.notify("warn", "No matches found to split on");
			}
			Err(e) => {
				self.notify("error", format!("Regex error: {}", e));
			}
		}
		false
	}

	pub(crate) fn do_split_lines(&mut self) -> bool {
		let primary = self.buffer().selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from >= to {
			self.notify("warn", "No selection to split");
			return false;
		}

		let start_line = self.buffer().doc.char_to_line(from);
		let end_line = self.buffer().doc.char_to_line(to.saturating_sub(1));

		let mut new_ranges: Vec<tome_base::range::Range> = Vec::new();
		for line in start_line..=end_line {
			let line_start = self.buffer().doc.line_to_char(line).max(from);
			let line_end = if line + 1 < self.buffer().doc.len_lines() {
				self.buffer().doc.line_to_char(line + 1).min(to)
			} else {
				self.buffer().doc.len_chars().min(to)
			};
			if line_start < line_end {
				new_ranges.push(tome_base::range::Range::new(line_start, line_end));
			}
		}

		if !new_ranges.is_empty() {
			self.buffer_mut().selection = Selection::from_vec(new_ranges, 0);
			self.notify("info", format!("{} lines", self.buffer().selection.len()));
		}
		false
	}

	#[allow(
		dead_code,
		reason = "keep-matching filter will be re-enabled via picker UI"
	)]
	pub(crate) fn keep_matching(&mut self, pattern: &str, invert: bool) -> bool {
		let mut kept_ranges: Vec<tome_base::range::Range> = Vec::new();
		let mut had_error = false;
		for range in self.buffer().selection.ranges() {
			let from = range.min();
			let to = range.max();
			let text: String = self.buffer().doc.slice(from..to).chars().collect();
			match movement::matches_pattern(&text, pattern) {
				Ok(matches) => {
					if matches != invert {
						kept_ranges.push(*range);
					}
				}
				Err(e) => {
					self.notify("error", format!("Regex error: {}", e));
					had_error = true;
					break;
				}
			}
		}

		if had_error {
			return false;
		}

		if kept_ranges.is_empty() {
			self.notify("warn", "No selections remain");
		} else {
			let count = kept_ranges.len();
			self.buffer_mut().selection = Selection::from_vec(kept_ranges, 0);
			self.notify("info", format!("{} selections kept", count));
		}
		false
	}
}
