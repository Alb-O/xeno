use super::entry::NotificationEntry;
use super::{AutoDismiss, Level};
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{
	FrozenInterner, LinkedDef, LinkedPayload, RegistryMeta, RegistryMetaStatic, RegistrySource,
	Symbol,
};

/// Static notification definition (for transition and direct use).
#[derive(Debug, Clone, Copy)]
pub struct NotificationDef {
	pub meta: RegistryMetaStatic,
	pub level: Level,
	pub auto_dismiss: AutoDismiss,
}

impl NotificationDef {
	pub const fn new(
		id: &'static str,
		level: Level,
		auto_dismiss: AutoDismiss,
		_source: RegistrySource,
	) -> Self {
		Self {
			meta: RegistryMetaStatic::minimal(id, "", ""), // Minimal meta for now
			level,
			auto_dismiss,
		}
	}
}

#[derive(Clone)]
pub struct NotificationPayload {
	pub level: Level,
	pub auto_dismiss: AutoDismiss,
}

impl LinkedPayload<NotificationEntry> for NotificationPayload {
	fn build_entry(
		&self,
		_interner: &FrozenInterner,
		meta: RegistryMeta,
		_short_desc: Symbol,
	) -> NotificationEntry {
		NotificationEntry {
			meta,
			level: self.level,
			auto_dismiss: self.auto_dismiss,
		}
	}
}

/// Linked notification definition assembled from KDL metadata.
pub type LinkedNotificationDef = LinkedDef<NotificationPayload>;

impl BuildEntry<NotificationEntry> for NotificationDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> NotificationEntry {
		let meta =
			crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

		NotificationEntry {
			meta,
			level: self.level,
			auto_dismiss: self.auto_dismiss,
		}
	}
}

pub type NotificationInput =
	crate::core::def_input::DefInput<NotificationDef, LinkedNotificationDef>;
