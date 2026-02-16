//! XENO_CTX schema for Nu macro/hook context injection.
//!
//! Defines the versioned record shape passed as `$env.XENO_CTX` to Nu
//! functions. The struct representation is the single source of truth;
//! [`NuCtx::to_value`] is the only place that constructs the Nu record.

use xeno_nu_value::{Record, Span, Value};

/// Current schema version. Bump when adding/removing/renaming fields.
pub const SCHEMA_VERSION: i64 = 1;

/// Snapshot of editor state passed to Nu functions as `$env.XENO_CTX`.
pub struct NuCtx {
	pub kind: String,
	pub function: String,
	pub args: Vec<String>,
	pub mode: String,
	pub view: NuCtxView,
	pub cursor: NuCtxPosition,
	pub selection: NuCtxSelection,
	pub buffer: NuCtxBuffer,
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
	pub start: NuCtxPosition,
	pub end: NuCtxPosition,
}

pub struct NuCtxBuffer {
	pub path: Option<String>,
	pub file_type: Option<String>,
	pub readonly: bool,
	pub modified: bool,
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
		selection.push("start", pos_record(&self.selection.start));
		selection.push("end", pos_record(&self.selection.end));

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
		ctx.push("args", Value::list(self.args.iter().map(|a| Value::string(a, s)).collect(), s));
		ctx.push("mode", Value::string(&self.mode, s));
		ctx.push("view", Value::record(view, s));
		ctx.push("cursor", Value::record(cursor, s));
		ctx.push("selection", Value::record(selection, s));
		ctx.push("buffer", Value::record(buffer, s));

		Value::record(ctx, s)
	}
}

#[cfg(test)]
mod tests;
