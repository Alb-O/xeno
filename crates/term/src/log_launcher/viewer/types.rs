use std::time::Instant;

use super::super::protocol::{Level, LogEvent, XenoLayer};

pub const MAX_LOG_ENTRIES: usize = 10000;
pub const HEADER_WIDTH: &str = "       ";
pub const PIPE: &str = "\x1b[90m│\x1b[0m ";

/// A stored log entry (event or span lifecycle).
#[derive(Clone)]
pub enum StoredEntry {
	Event {
		event: LogEvent,
		relative_ms: u64,
	},
	SpanEnter {
		name: String,
		target: String,
		level: Level,
		layer: XenoLayer,
		fields: Vec<(String, String)>,
		depth: usize,
		relative_ms: u64,
	},
	SpanClose {
		name: String,
		level: Level,
		layer: XenoLayer,
		duration_us: u64,
		depth: usize,
	},
}

impl StoredEntry {
	pub fn level(&self) -> Level {
		match self {
			StoredEntry::Event { event, .. } => event.level,
			StoredEntry::SpanEnter { level, .. } | StoredEntry::SpanClose { level, .. } => *level,
		}
	}

	pub fn layer(&self) -> XenoLayer {
		match self {
			StoredEntry::Event { event, .. } => event.layer,
			StoredEntry::SpanEnter { layer, .. } | StoredEntry::SpanClose { layer, .. } => *layer,
		}
	}

	pub fn target(&self) -> &str {
		match self {
			StoredEntry::Event { event, .. } => &event.target,
			StoredEntry::SpanEnter { target, .. } => target,
			StoredEntry::SpanClose { .. } => "",
		}
	}
}

/// Tracked editor statistics from editor.stats events.
#[derive(Debug, Default, Clone)]
pub struct EditorStats {
	pub hooks_pending: u64,
	pub hooks_scheduled: u64,
	pub hooks_completed: u64,
	pub lsp_pending_docs: u64,
	pub lsp_in_flight: u64,
	pub lsp_full_sync: u64,
	pub lsp_incremental_sync: u64,
	pub lsp_send_errors: u64,
	pub lsp_coalesced: u64,
	pub last_updated: Option<Instant>,
}

impl EditorStats {
	pub fn update_from_fields(&mut self, fields: &[(String, String)]) {
		for (key, value) in fields {
			if let Ok(v) = value.parse::<u64>() {
				match key.as_str() {
					"hooks_pending" => self.hooks_pending = v,
					"hooks_scheduled" => self.hooks_scheduled = v,
					"hooks_completed" => self.hooks_completed = v,
					"lsp_pending_docs" => self.lsp_pending_docs = v,
					"lsp_in_flight" => self.lsp_in_flight = v,
					"lsp_full_sync" => self.lsp_full_sync = v,
					"lsp_incremental_sync" => self.lsp_incremental_sync = v,
					"lsp_send_errors" => self.lsp_send_errors = v,
					"lsp_coalesced" => self.lsp_coalesced = v,
					_ => {}
				}
			}
		}
		self.last_updated = Some(Instant::now());
	}
}

pub struct ActiveSpan {
	pub name: String,
	#[allow(dead_code)]
	pub target: String,
	pub level: Level,
	pub layer: XenoLayer,
	pub depth: usize,
}
