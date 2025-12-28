use tome_base::Selection;
use tome_stdlib::movement;

use super::Editor;

impl Editor {
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
				movement::find_next(doc.content.slice(..), &pattern, cursor_pos + 1)
			};
			match search_result {
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
				movement::find_prev(doc.content.slice(..), &pattern, cursor_pos)
			};
			match search_result {
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
			let (text, pattern) = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				let text: String = doc.content.slice(from..to).chars().collect();
				let pattern = movement::escape_pattern(&text);
				(text, pattern)
			};
			self.buffer_mut()
				.input
				.set_last_search(pattern.clone(), false);
			self.notify("info", format!("Search: {}", text));
			let search_result = {
				let buffer = self.buffer();
				let doc = buffer.doc();
				movement::find_next(doc.content.slice(..), &pattern, to)
			};
			match search_result {
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

		let search_result = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			movement::find_all_matches(doc.content.slice(from..to), pattern)
		};
		match search_result {
			Ok(matches) if !matches.is_empty() => {
				let new_ranges: Vec<tome_base::range::Range> = matches
					.into_iter()
					.map(|r| tome_base::range::Range::new(from + r.min(), from + r.max()))
					.collect();
				let count = new_ranges.len();
				self.buffer_mut().selection = Selection::from_vec(new_ranges, 0);
				self.notify("info", format!("{} matches", count));
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

		let search_result = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			movement::find_all_matches(doc.content.slice(from..to), pattern)
		};
		match search_result {
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
					let count = new_ranges.len();
					self.buffer_mut().selection = Selection::from_vec(new_ranges, 0);
					self.notify("info", format!("{} splits", count));
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

	#[allow(
		dead_code,
		reason = "keep-matching filter will be re-enabled via picker UI"
	)]
	pub(crate) fn keep_matching(&mut self, pattern: &str, invert: bool) -> bool {
		// Collect ranges and text to process
		let ranges_with_text: Vec<(tome_base::range::Range, String)> = {
			let buffer = self.buffer();
			let doc = buffer.doc();
			buffer
				.selection
				.ranges()
				.iter()
				.map(|range| {
					let from = range.min();
					let to = range.max();
					let text: String = doc.content.slice(from..to).chars().collect();
					(*range, text)
				})
				.collect()
		};

		let mut kept_ranges: Vec<tome_base::range::Range> = Vec::new();
		let mut had_error = false;
		for (range, text) in ranges_with_text {
			match movement::matches_pattern(&text, pattern) {
				Ok(matches) => {
					if matches != invert {
						kept_ranges.push(range);
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
