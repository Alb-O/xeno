use ropey::RopeSlice;

/// Configuration for viewport repair scanning.
#[derive(Debug, Clone)]
pub struct ViewportRepair {
	pub enabled: bool,
	pub max_scan_bytes: u32,
	pub prefer_real_closer: bool,
	pub max_forward_search_bytes: u32,
	pub rules: Vec<ViewportRepairRule>,
}

/// A rule for detecting unclosed constructs in viewport windows.
#[derive(Debug, Clone)]
pub enum ViewportRepairRule {
	BlockComment { open: String, close: String, nestable: bool },
	String { quote: String, escape: Option<String> },
	LineComment { start: String },
}

/// Result of a viewport repair scan.
#[derive(Debug, Clone, Default)]
pub struct SealPlan {
	/// Synthetic suffix to append to the window.
	pub suffix: String,
	/// Number of bytes to extend the window by from the forward haystack.
	pub extension_bytes: u32,
}

impl ViewportRepair {
	/// Scans the window to determine the synthetic suffix or extension needed to close multi-line constructs.
	///
	/// Optionally performs a forward search in the full document to find a real closer.
	pub fn scan(&self, window: RopeSlice<'_>, forward_haystack: Option<RopeSlice<'_>>) -> SealPlan {
		if !self.enabled || window.len_bytes() == 0 {
			return SealPlan::default();
		}

		// MVP byte-oriented scanner
		let mut block_comment_depth = 0;
		let mut in_string: Option<usize> = None; // index into self.rules
		let mut in_line_comment = false;

		// Use a chunk-based iterator to avoid large Vec allocations
		let total_bytes = window.len_bytes().min(self.max_scan_bytes as usize);
		let mut bytes_read = 0;

		'outer: for chunk in window.chunks() {
			let chunk_bytes = chunk.as_bytes();
			let mut chunk_idx = 0;

			while chunk_idx < chunk_bytes.len() && bytes_read < total_bytes {
				if in_line_comment {
					if chunk_bytes[chunk_idx] == b'\n' {
						in_line_comment = false;
					}
					chunk_idx += 1;
					bytes_read += 1;
					continue;
				}

				if let Some(rule_idx) = in_string {
					let rule = &self.rules[rule_idx];
					if let ViewportRepairRule::String { quote, escape } = rule {
						if let Some(esc) = escape
							&& chunk_bytes[chunk_idx..].starts_with(esc.as_bytes())
						{
							chunk_idx += esc.len();
							bytes_read += esc.len();
							if chunk_idx < chunk_bytes.len() {
								chunk_idx += 1;
								bytes_read += 1;
							}
							continue;
						}
						if chunk_bytes[chunk_idx..].starts_with(quote.as_bytes()) {
							in_string = None;
							chunk_idx += quote.len();
							bytes_read += quote.len();
							continue;
						}
					}
					chunk_idx += 1;
					bytes_read += 1;
					continue;
				}

				// Not in line comment or string
				let mut matched = false;
				for (idx, rule) in self.rules.iter().enumerate() {
					match rule {
						ViewportRepairRule::LineComment { start } => {
							if chunk_bytes[chunk_idx..].starts_with(start.as_bytes()) {
								in_line_comment = true;
								chunk_idx += start.len();
								bytes_read += start.len();
								matched = true;
								break;
							}
						}
						ViewportRepairRule::String { quote, .. } => {
							if chunk_bytes[chunk_idx..].starts_with(quote.as_bytes()) {
								in_string = Some(idx);
								chunk_idx += quote.len();
								bytes_read += quote.len();
								matched = true;
								break;
							}
						}
						ViewportRepairRule::BlockComment { open, close, nestable } => {
							if chunk_bytes[chunk_idx..].starts_with(open.as_bytes()) {
								block_comment_depth += 1;
								chunk_idx += open.len();
								bytes_read += open.len();
								matched = true;
								if !*nestable {
									// skip until closer in the same chunk (simplified)
									// FIXME: this should handle closer spanning chunks
									while chunk_idx < chunk_bytes.len() && bytes_read < total_bytes {
										if chunk_bytes[chunk_idx..].starts_with(close.as_bytes()) {
											block_comment_depth -= 1;
											chunk_idx += close.len();
											bytes_read += close.len();
											break;
										}
										chunk_idx += 1;
										bytes_read += 1;
									}
								}
								break;
							} else if chunk_bytes[chunk_idx..].starts_with(close.as_bytes()) {
								if block_comment_depth > 0 {
									block_comment_depth -= 1;
								}
								chunk_idx += close.len();
								bytes_read += close.len();
								matched = true;
								break;
							}
						}
					}
				}

				if !matched {
					chunk_idx += 1;
					bytes_read += 1;
				}
			}

			if bytes_read >= total_bytes {
				break 'outer;
			}
		}

		if block_comment_depth == 0 && in_string.is_none() {
			return SealPlan::default();
		}

		// Check for real closer forward if requested
		if self.prefer_real_closer
			&& let Some(haystack) = forward_haystack
		{
			let search_limit = self.max_forward_search_bytes as usize;
			let mut search_bytes_read = 0;

			for chunk in haystack.chunks() {
				let chunk_bytes = chunk.as_bytes();
				let current_limit = (search_limit - search_bytes_read).min(chunk_bytes.len());

				if block_comment_depth > 0 {
					if let Some(ViewportRepairRule::BlockComment { close, .. }) =
						self.rules.iter().find(|r| matches!(r, ViewportRepairRule::BlockComment { .. }))
						&& let Some(pos) = chunk_bytes[..current_limit].windows(close.len()).position(|w| w == close.as_bytes())
					{
						return SealPlan {
							suffix: String::new(),
							extension_bytes: (search_bytes_read + pos + close.len()) as u32,
						};
					}
				} else if let Some(rule_idx) = in_string
					&& let ViewportRepairRule::String { quote, .. } = &self.rules[rule_idx]
					&& let Some(pos) = chunk_bytes[..current_limit].windows(quote.len()).position(|w| w == quote.as_bytes())
				{
					return SealPlan {
						suffix: String::new(),
						extension_bytes: (search_bytes_read + pos + quote.len()) as u32,
					};
				}

				search_bytes_read += current_limit;
				if search_bytes_read >= search_limit {
					break;
				}
			}
		}

		let mut suffix = String::new();
		if block_comment_depth > 0 {
			// find first block comment rule to get closer
			if let Some(ViewportRepairRule::BlockComment { close, .. }) = self.rules.iter().find(|r| matches!(r, ViewportRepairRule::BlockComment { .. })) {
				for _ in 0..block_comment_depth {
					suffix.push_str(close);
				}
			}
		} else if let Some(rule_idx) = in_string
			&& let ViewportRepairRule::String { quote, .. } = &self.rules[rule_idx]
		{
			suffix.push_str(quote);
		}

		SealPlan { suffix, extension_bytes: 0 }
	}
}
