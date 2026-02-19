//! XENO_CTX schema for Nu macro/hook context injection.
//!
//! Defines the versioned record shape passed as `$env.XENO_CTX` to Nu
//! functions. The struct representation is the single source of truth;
//! [`NuCtx::to_value`] is the only place that constructs the Nu record.

use xeno_nu_data::{Record, Span, Value};

/// Current schema version. Bump when adding/removing/renaming fields.
pub const SCHEMA_VERSION: i64 = 7;

/// Max byte length for text snapshots (cursor line, selection text).
///
/// Uses the same cap as invocation string limits for consistency.
pub const TEXT_SNAPSHOT_MAX_BYTES: usize = xeno_invocation::schema::DEFAULT_LIMITS.max_string_len;

/// Snapshot of editor state passed to Nu functions as `$env.XENO_CTX`.
pub struct NuCtx {
	pub kind: String,
	pub function: String,
	pub mode: String,
	pub view: NuCtxView,
	pub cursor: NuCtxPosition,
	pub selection: NuCtxSelection,
	pub buffer: NuCtxBuffer,
	pub text: NuCtxText,
	pub event: Option<NuCtxEvent>,
	pub state: Vec<(String, String)>,
}

pub struct NuCtxView {
	pub id: u64,
}

pub struct NuCtxPosition {
	pub line: usize,
	pub col: usize,
}

pub struct NuCtxSelection {
	pub active: bool,
	/// Index of the primary range in `ranges`.
	pub primary: usize,
	/// Primary range min position (backward compat).
	pub start: NuCtxPosition,
	/// Primary range max position (backward compat).
	pub end: NuCtxPosition,
	/// All selection ranges with anchor/head direction preserved.
	pub ranges: Vec<NuCtxRange>,
}

pub struct NuCtxRange {
	pub anchor: NuCtxPosition,
	pub head: NuCtxPosition,
}

pub struct NuCtxBuffer {
	pub path: Option<String>,
	pub file_type: Option<String>,
	pub readonly: bool,
	pub modified: bool,
}

/// Text snapshot from the buffer at the cursor/selection.
///
/// Populated only for macro invocations (hooks get null/false defaults)
/// to avoid per-action extraction cost on high-frequency hook calls.
pub struct NuCtxText {
	pub line: Option<String>,
	pub line_truncated: bool,
	pub selection: Option<String>,
	pub selection_truncated: bool,
}

impl NuCtxText {
	/// Empty snapshot used for hook invocations.
	pub fn empty() -> Self {
		Self {
			line: None,
			line_truncated: false,
			selection: None,
			selection_truncated: false,
		}
	}
}

/// Structured hook event data, replacing positional-arg dependence.
///
/// Populated for hook invocations so scripts can inspect event details
/// via `$ctx.event.type` and `$ctx.event.data` instead of relying on
/// positional arguments. Macros receive `null`.
#[derive(Debug, Clone)]
pub enum NuCtxEvent {
	ActionPost { name: String, result: String },
	CommandPost { name: String, result: String, args: Vec<String> },
	EditorCommandPost { name: String, result: String, args: Vec<String> },
	ModeChange { from: String, to: String },
	BufferOpen { path: String, kind: String },
}

impl NuCtxEvent {
	/// Returns true if two events are the same kind (used for queue coalescing).
	pub(crate) fn same_kind(&self, other: &Self) -> bool {
		std::mem::discriminant(self) == std::mem::discriminant(other)
	}

	pub(crate) fn type_str(&self) -> &'static str {
		match self {
			Self::ActionPost { .. } => "action_post",
			Self::CommandPost { .. } => "command_post",
			Self::EditorCommandPost { .. } => "editor_command_post",
			Self::ModeChange { .. } => "mode_change",
			Self::BufferOpen { .. } => "buffer_open",
		}
	}

	fn to_value(&self, s: Span) -> Value {
		let mut data = Record::new();
		match self {
			Self::ActionPost { name, result } => {
				data.push("name", Value::string(name, s));
				data.push("result", Value::string(result, s));
			}
			Self::CommandPost { name, result, args } | Self::EditorCommandPost { name, result, args } => {
				data.push("name", Value::string(name, s));
				data.push("result", Value::string(result, s));
				data.push("args", Value::list(args.iter().map(|a| Value::string(a, s)).collect(), s));
			}
			Self::ModeChange { from, to } => {
				data.push("from", Value::string(from, s));
				data.push("to", Value::string(to, s));
			}
			Self::BufferOpen { path, kind } => {
				data.push("path", Value::string(path, s));
				data.push("kind", Value::string(kind, s));
			}
		}
		let mut rec = Record::new();
		rec.push("type", Value::string(self.type_str(), s));
		rec.push("data", Value::record(data, s));
		Value::record(rec, s)
	}
}

/// Truncate a string at a UTF-8 safe boundary.
///
/// Returns the (possibly shortened) string and whether truncation occurred.
#[cfg(test)]
pub(crate) fn clamp_utf8(s: &str, max_bytes: usize) -> (String, bool) {
	if s.len() <= max_bytes {
		return (s.to_string(), false);
	}
	let mut end = max_bytes;
	while end > 0 && !s.is_char_boundary(end) {
		end -= 1;
	}
	(s[..end].to_string(), true)
}

/// Extract text from a rope slice with a hard byte cap, streaming chunks.
///
/// Avoids allocating the full slice contents when the slice is larger than
/// `max_bytes`. Iterates rope chunks and stops as soon as the budget is
/// exhausted, truncating at a UTF-8 char boundary.
pub(crate) fn rope_slice_clamped(slice: xeno_primitives::RopeSlice<'_>, max_bytes: usize) -> (String, bool) {
	if max_bytes == 0 {
		return (String::new(), slice.len_bytes() > 0);
	}
	let total = slice.len_bytes();
	if total <= max_bytes {
		return (String::from(slice), false);
	}
	let mut out = String::with_capacity(max_bytes.min(256));
	let mut remaining = max_bytes;
	let mut chunks = slice.chunks();
	while let Some(chunk) = chunks.next() {
		if chunk.len() <= remaining {
			out.push_str(chunk);
			remaining -= chunk.len();
			if remaining == 0 {
				let truncated = chunks.next().is_some();
				return (out, truncated);
			}
		} else {
			let mut end = remaining;
			while end > 0 && !chunk.is_char_boundary(end) {
				end -= 1;
			}
			out.push_str(&chunk[..end]);
			return (out, true);
		}
	}
	(out, false)
}

impl NuCtx {
	/// Convert to a Nu `Value::Record` for injection as `$env.XENO_CTX`.
	pub fn to_value(&self) -> Value {
		let s = Span::unknown();

		let int = |v: usize| Value::int(v.min(i64::MAX as usize) as i64, s);
		let int_u64 = |v: u64| Value::int(v.min(i64::MAX as u64) as i64, s);

		let mut view = Record::new();
		view.push("id", int_u64(self.view.id));

		let mut cursor = Record::new();
		cursor.push("line", int(self.cursor.line));
		cursor.push("col", int(self.cursor.col));

		let pos_record = |p: &NuCtxPosition| {
			let mut r = Record::new();
			r.push("line", int(p.line));
			r.push("col", int(p.col));
			Value::record(r, s)
		};

		let mut selection = Record::new();
		selection.push("active", Value::bool(self.selection.active, s));
		selection.push("primary", int(self.selection.primary));
		selection.push("start", pos_record(&self.selection.start));
		selection.push("end", pos_record(&self.selection.end));
		let ranges: Vec<Value> = self
			.selection
			.ranges
			.iter()
			.map(|r| {
				let mut rec = Record::new();
				rec.push("anchor", pos_record(&r.anchor));
				rec.push("head", pos_record(&r.head));
				Value::record(rec, s)
			})
			.collect();
		selection.push("ranges", Value::list(ranges, s));

		let mut buffer = Record::new();
		buffer.push("path", self.buffer.path.as_ref().map_or_else(|| Value::nothing(s), |p| Value::string(p, s)));
		buffer.push(
			"file_type",
			self.buffer.file_type.as_ref().map_or_else(|| Value::nothing(s), |ft| Value::string(ft, s)),
		);
		buffer.push("readonly", Value::bool(self.buffer.readonly, s));
		buffer.push("modified", Value::bool(self.buffer.modified, s));

		let mut ctx = Record::new();
		ctx.push("schema_version", Value::int(SCHEMA_VERSION, s));
		ctx.push("kind", Value::string(&self.kind, s));
		ctx.push("function", Value::string(&self.function, s));
		ctx.push("mode", Value::string(&self.mode, s));
		ctx.push("view", Value::record(view, s));
		ctx.push("cursor", Value::record(cursor, s));
		ctx.push("selection", Value::record(selection, s));
		ctx.push("buffer", Value::record(buffer, s));

		let opt_str = |v: &Option<String>| v.as_ref().map_or_else(|| Value::nothing(s), |v| Value::string(v, s));
		let mut text = Record::new();
		text.push("line", opt_str(&self.text.line));
		text.push("line_truncated", Value::bool(self.text.line_truncated, s));
		text.push("selection", opt_str(&self.text.selection));
		text.push("selection_truncated", Value::bool(self.text.selection_truncated, s));
		ctx.push("text", Value::record(text, s));
		ctx.push("event", self.event.as_ref().map_or_else(|| Value::nothing(s), |e| e.to_value(s)));
		let mut state = Record::with_capacity(self.state.len());
		for (k, v) in &self.state {
			state.push(k.clone(), Value::string(v, s));
		}
		ctx.push("state", Value::record(state, s));

		Value::record(ctx, s)
	}
}

#[cfg(test)]
mod tests;
