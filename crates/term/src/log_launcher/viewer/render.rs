use std::io::{self, Write};

use crossterm::terminal::{self, ClearType};
use crossterm::{cursor, queue};

use super::LogViewer;
use super::format::{cyan, dim, format_duration, format_relative_time, truncate_target};
use super::types::{HEADER_WIDTH, PIPE, StoredEntry};

impl LogViewer {
	pub fn format_entry(&self, entry: &StoredEntry) -> Vec<String> {
		match entry {
			StoredEntry::Event { event, relative_ms } => {
				let indent = self.line_prefix(event.spans.len(), false);
				let cont = self.line_prefix(event.spans.len(), true);
				let ts = format_relative_time(*relative_ms);
				let target = truncate_target(&event.target);
				let mut lines = vec![format!(
					"{}{} {} {}",
					indent,
					dim(&ts),
					event.level.colored(),
					event.layer.colored()
				)];
				for (i, msg_line) in event.message.lines().enumerate() {
					if i == 0 {
						lines.push(format!("{}{} > {}", cont, dim(&target), msg_line));
					} else {
						lines.push(format!("{}  {}", cont, msg_line));
					}
				}
				let fields: Vec<_> = event
					.fields
					.iter()
					.filter(|(k, _)| !k.starts_with("log."))
					.collect();
				let max_key = fields.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
				for (key, value) in fields {
					let value = value.replace('\n', " ");
					lines.push(format!(
						"{}    {} = {}",
						cont,
						dim(&format!("{:>width$}", key, width = max_key)),
						value
					));
				}
				lines
			}
			StoredEntry::SpanEnter {
				name,
				target,
				level,
				layer,
				fields,
				depth,
				relative_ms,
			} => {
				let indent = self.line_prefix(*depth, false);
				let cont = self.line_prefix(*depth, true);
				let ts = format_relative_time(*relative_ms);
				let target = truncate_target(target);
				let mut lines = vec![
					format!(
						"{}{} {} {}",
						indent,
						dim(&ts),
						level.colored(),
						layer.colored()
					),
					format!("{}{} {}", cont, dim(&target), cyan(name)),
				];
				let fields: Vec<_> = fields
					.iter()
					.filter(|(k, _)| !k.starts_with("log."))
					.collect();
				let max_key = fields.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
				for (key, value) in fields {
					let value = value.replace('\n', " ");
					lines.push(format!(
						"{}    {} = {}",
						cont,
						dim(&format!("{:>width$}", key, width = max_key)),
						value
					));
				}
				lines
			}
			StoredEntry::SpanClose {
				name,
				duration_us,
				depth,
				..
			} => {
				let duration = format_duration(*duration_us);
				let prefix = self.line_prefix(*depth, true);
				vec![format!("{}← {} {}", prefix, cyan(name), dim(&duration))]
			}
		}
	}

	pub fn render(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
		queue!(stdout, cursor::MoveTo(0, 0))?;

		let content_height = self.term_height.saturating_sub(1) as usize;
		let all_lines: Vec<String> = self
			.entries
			.iter()
			.filter(|e| self.matches_filter(e))
			.flat_map(|e| self.format_entry(e))
			.collect();

		let total_lines = all_lines.len();
		let visible_entries = self
			.entries
			.iter()
			.filter(|e| self.matches_filter(e))
			.count();

		if total_lines == 0 {
			for row in 0..content_height {
				queue!(stdout, cursor::MoveTo(0, row as u16))?;
				queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;
			}
			self.render_status_bar(stdout, visible_entries)?;
			return stdout.flush();
		}

		let max_offset = total_lines.saturating_sub(content_height);
		self.scroll_offset = self.scroll_offset.min(max_offset);

		let start = max_offset.saturating_sub(self.scroll_offset);
		let end = (start + content_height).min(total_lines);

		for (row, line) in all_lines[start..end].iter().enumerate() {
			queue!(stdout, cursor::MoveTo(0, row as u16))?;
			queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;
			write!(stdout, "{}", line)?;
		}

		for row in (end - start)..content_height {
			queue!(stdout, cursor::MoveTo(0, row as u16))?;
			queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;
		}

		self.render_status_bar(stdout, visible_entries)?;
		if self.show_help {
			self.render_help(stdout)?;
		}
		if self.show_stats {
			self.render_stats(stdout)?;
		}

		stdout.flush()
	}

	pub fn line_prefix(&self, depth: usize, continuation: bool) -> String {
		if depth == 0 && !continuation {
			return String::new();
		}
		let mut prefix = String::from(HEADER_WIDTH);
		for i in 0..depth {
			prefix.push_str(PIPE);
			if i < depth - 1 || continuation {
				prefix.push_str(HEADER_WIDTH);
			}
		}
		if continuation {
			prefix.push_str(PIPE);
		}
		prefix
	}

	pub fn render_status_bar(&self, stdout: &mut io::Stdout, total_lines: usize) -> io::Result<()> {
		queue!(stdout, cursor::MoveTo(0, self.term_height - 1))?;
		queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;

		let level = match self.min_level {
			super::super::protocol::Level::Trace => "T",
			super::super::protocol::Level::Debug => "D",
			super::super::protocol::Level::Info => "I",
			super::super::protocol::Level::Warn => "W",
			super::super::protocol::Level::Error => "E",
		};
		let status = if self.disconnected {
			"\x1b[31m EXITED \x1b[0m\x1b[7m "
		} else if self.paused {
			"PAUSED "
		} else {
			""
		};
		let scroll = if self.scroll_offset > 0 {
			format!(" [+{}]", self.scroll_offset)
		} else {
			String::new()
		};
		let layer_str = if self.layer_filter.is_empty() {
			String::new()
		} else {
			let layers: Vec<_> = self.layer_filter.iter().map(|l| l.short_name()).collect();
			format!(" [{}]", layers.join(","))
		};
		let filter = if self.target_filter.is_empty() {
			String::new()
		} else {
			format!(" filter:{}", self.target_filter)
		};

		write!(
			stdout,
			"\x1b[7m {}{} {} lines{}{}{} | ? help | q quit \x1b[0m",
			status, level, total_lines, layer_str, scroll, filter
		)
	}

	pub fn render_help(&self, stdout: &mut io::Stdout) -> io::Result<()> {
		const HELP: &[&str] = &[
			"",
			"  Log Viewer Controls",
			"  ───────────────────",
			"",
			"  Level Filters:",
			"    t  Show TRACE and above",
			"    d  Show DEBUG and above",
			"    i  Show INFO and above",
			"    w  Show WARN and above",
			"    e  Show ERROR only",
			"",
			"  Layer Filters (toggle):",
			"    1 CORE  2 API   3 LSP   4 LANG  5 CFG",
			"    6 UI    7 REG   8 EXT",
			"    L       Clear layer filter (show all)",
			"",
			"  Navigation:",
			"    j/Down    Scroll down",
			"    k/Up      Scroll up",
			"    g/Home    Go to top",
			"    G/End     Go to bottom",
			"    Space     Toggle pause",
			"",
			"  Other:",
			"    s         Toggle stats overlay",
			"    c         Clear logs",
			"    /         Set target filter",
			"    Esc       Clear filter / close help",
			"    q         Quit",
			"",
		];

		let box_width = 45usize;
		let start_col = (self.term_width / 2).saturating_sub(box_width as u16 / 2);
		let start_row = (self.term_height / 2).saturating_sub(HELP.len() as u16 / 2);

		for (i, line) in HELP.iter().enumerate() {
			queue!(stdout, cursor::MoveTo(start_col, start_row + i as u16))?;
			write!(
				stdout,
				"\x1b[44;97m{:width$}\x1b[0m",
				line,
				width = box_width
			)?;
		}

		Ok(())
	}

	pub fn render_stats(&self, stdout: &mut io::Stdout) -> io::Result<()> {
		let age = self
			.stats
			.last_updated
			.map(|t| {
				let secs = t.elapsed().as_secs();
				if secs < 60 {
					format!("{}s ago", secs)
				} else {
					format!("{}m ago", secs / 60)
				}
			})
			.unwrap_or_else(|| "never".to_string());

		let lines: Vec<String> = vec![
			String::new(),
			"  Editor Statistics".to_string(),
			"  ─────────────────".to_string(),
			String::new(),
			"  Hooks:".to_string(),
			format!("    Pending:   {:>8}", self.stats.hooks_pending),
			format!("    Scheduled: {:>8}", self.stats.hooks_scheduled),
			format!("    Completed: {:>8}", self.stats.hooks_completed),
			String::new(),
			"  LSP Sync:".to_string(),
			format!("    Pending:     {:>6}", self.stats.lsp_pending_docs),
			format!("    In-flight:   {:>6}", self.stats.lsp_in_flight),
			format!("    Full sync:   {:>6}", self.stats.lsp_full_sync),
			format!("    Incremental: {:>6}", self.stats.lsp_incremental_sync),
			format!("    Errors:      {:>6}", self.stats.lsp_send_errors),
			format!("    Coalesced:   {:>6}", self.stats.lsp_coalesced),
			String::new(),
			format!("  Updated: {}", age),
			String::new(),
		];

		let box_width = 30usize;
		let start_col = self.term_width.saturating_sub(box_width as u16 + 2);
		let start_row = 1u16;

		for (i, line) in lines.iter().enumerate() {
			queue!(stdout, cursor::MoveTo(start_col, start_row + i as u16))?;
			write!(
				stdout,
				"\x1b[42;30m{:width$}\x1b[0m",
				line,
				width = box_width
			)?;
		}

		Ok(())
	}
}
