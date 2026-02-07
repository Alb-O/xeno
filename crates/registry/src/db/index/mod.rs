//! Registry indexing and lookup for editor extensions.

pub mod collision;
pub mod diagnostics;
pub mod lookups;

pub use collision::CollisionKind;
pub use diagnostics::{DiagnosticReport, diagnostics};
pub use lookups::{
	all_actions, all_commands, all_motions, all_text_objects, find_action, find_action_by_id,
	find_command, find_motion, find_text_object_by_trigger, resolve_action_id, resolve_action_key,
};
