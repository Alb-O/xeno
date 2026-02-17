//! Xeno safe Nu stdlib commands.
//!
//! A curated allowlist of Nu commands safe for sandboxed evaluation.
//! Commands are either ported from upstream `nu-command` (with imports
//! adapted) or written as xeno-owned minimal implementations.
//!
//! Public API: only [`register_all`] is exported. All command implementations,
//! helpers, and limits are internal. Sandbox caps: `MAX_ITEMS` (10 000),
//! `MAX_COLUMNS` (128), `MAX_SPLITS` (10 000).
#![allow(clippy::result_large_err, reason = "ShellError is intentionally rich and shared across Nu command APIs")]
#![allow(clippy::collapsible_if, reason = "Some command logic keeps nested guards for readability and traceability")]
#![allow(
	clippy::redundant_locals,
	reason = "Local rebinding is used in closures to make ownership and move points explicit"
)]

mod conversions;
mod filters;
pub(crate) mod limits;
pub(crate) mod strings;

use xeno_nu_protocol::engine::StateWorkingSet;

/// Registers all safe stdlib commands into the given working set.
pub fn register_all(working_set: &mut StateWorkingSet<'_>) {
	working_set.add_decl(Box::new(filters::Append));
	working_set.add_decl(Box::new(filters::Compact));
	working_set.add_decl(Box::new(filters::Each));
	working_set.add_decl(Box::new(filters::Flatten));
	working_set.add_decl(Box::new(filters::Get));
	working_set.add_decl(Box::new(filters::IsEmpty));
	working_set.add_decl(Box::new(filters::Length));
	working_set.add_decl(Box::new(filters::Prepend));
	working_set.add_decl(Box::new(filters::Reduce));
	working_set.add_decl(Box::new(filters::Reject));
	working_set.add_decl(Box::new(filters::Select));
	working_set.add_decl(Box::new(filters::Sort));
	working_set.add_decl(Box::new(filters::SortBy));
	working_set.add_decl(Box::new(filters::Update));
	working_set.add_decl(Box::new(filters::Upsert));
	working_set.add_decl(Box::new(filters::Where));
	working_set.add_decl(Box::new(strings::SplitRow));
	working_set.add_decl(Box::new(strings::StrContains));
	working_set.add_decl(Box::new(strings::StrDowncase));
	working_set.add_decl(Box::new(strings::StrEndsWith));
	working_set.add_decl(Box::new(strings::StrReplace));
	working_set.add_decl(Box::new(strings::StrStartsWith));
	working_set.add_decl(Box::new(strings::StrTrim));
	working_set.add_decl(Box::new(strings::StrUpcase));
	working_set.add_decl(Box::new(conversions::IntoBool));
	working_set.add_decl(Box::new(conversions::IntoInt));
	working_set.add_decl(Box::new(conversions::IntoString));
}
