use crate::ext::ExtensionMetadata;
use crate::ext::index::ExtensionRegistry;
use crate::ext::index::collision::CollisionKind;

pub struct DiagnosticReport {
	pub collisions: Vec<CollisionReport>,
}

pub struct CollisionReport {
	pub kind: CollisionKind,
	pub key: String,
	pub winner_id: &'static str,
	pub winner_source: String,
	pub winner_priority: i16,
	pub shadowed_id: &'static str,
	pub shadowed_source: String,
	pub shadowed_priority: i16,
}

pub(crate) fn diagnostics_internal(reg: &ExtensionRegistry) -> DiagnosticReport {
	let mut reports = Vec::new();

	macro_rules! collect {
		($index:expr) => {
			for c in &$index.collisions {
				reports.push(CollisionReport {
					kind: c.kind,
					key: c.key.clone(),
					winner_id: c.winner.id,
					winner_source: c.winner.source.to_string(),
					winner_priority: c.winner.priority,
					shadowed_id: c.shadowed.id,
					shadowed_source: c.shadowed.source.to_string(),
					shadowed_priority: c.shadowed.priority,
				});
			}
		};
	}

	collect!(reg.commands);
	collect!(reg.actions);
	collect!(reg.motions);
	collect!(reg.text_objects);
	collect!(reg.file_types);

	DiagnosticReport {
		collisions: reports,
	}
}

pub fn diagnostics() -> DiagnosticReport {
	diagnostics_internal(super::get_registry())
}
