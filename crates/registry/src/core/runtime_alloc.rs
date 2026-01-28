//! Allocation helpers for runtime registry extensions.

/// Leaks a string to create a static reference.
pub fn leak_str(s: String) -> &'static str {
	Box::leak(s.into_boxed_str())
}

/// Leaks a collection of strings to create a static slice of static strings.
pub fn leak_strs(v: Vec<String>) -> &'static [&'static str] {
	let leaked: Vec<&'static str> = v.into_iter().map(leak_str).collect();
	Box::leak(leaked.into_boxed_slice())
}

/// Helper for building registry metadata at runtime without manual leaking.
pub struct RuntimeMetaBuilder {
	id: String,
	name: String,
	description: String,
	aliases: Vec<String>,
	priority: i16,
	caps: Vec<crate::Capability>,
	flags: u32,
}

impl RuntimeMetaBuilder {
	pub fn new(
		id: impl Into<String>,
		name: impl Into<String>,
		description: impl Into<String>,
	) -> Self {
		Self {
			id: id.into(),
			name: name.into(),
			description: description.into(),
			aliases: Vec::new(),
			priority: 0,
			caps: Vec::new(),
			flags: 0,
		}
	}

	pub fn alias(mut self, alias: impl Into<String>) -> Self {
		self.aliases.push(alias.into());
		self
	}

	pub fn priority(mut self, p: i16) -> Self {
		self.priority = p;
		self
	}

	pub fn capability(mut self, cap: crate::Capability) -> Self {
		self.caps.push(cap);
		self
	}

	pub fn flags(mut self, flags: u32) -> Self {
		self.flags = flags;
		self
	}

	pub fn build(self) -> crate::RegistryMeta {
		crate::RegistryMeta {
			id: leak_str(self.id),
			name: leak_str(self.name),
			aliases: leak_strs(self.aliases),
			description: leak_str(self.description),
			priority: self.priority,
			source: crate::RegistrySource::Runtime,
			required_caps: Box::leak(self.caps.into_boxed_slice()),
			flags: self.flags,
		}
	}
}
