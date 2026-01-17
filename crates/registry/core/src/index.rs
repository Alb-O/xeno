//! Centralized registry index infrastructure.
//!
//! Provides [`RegistryBuilder`] and [`RegistryIndex`] to eliminate boilerplate
//! across registries. Each registry uses the same pattern:
//!
//! ```rust,ignore
//! static MOTIONS: LazyLock<RegistryIndex<MotionDef>> = LazyLock::new(|| {
//!     RegistryBuilder::new("motions")
//!         .extend_inventory::<MotionReg>()
//!         .sort_by(|a, b| a.name().cmp(b.name()))
//!         .build()
//! });
//! ```

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::RegistryEntry;

/// Trait for inventory wrapper types to expose their definition.
///
/// Implement this for your registry's wrapper type (e.g., `MotionReg`)
/// to allow [`RegistryBuilder::extend_inventory`] to extract definitions.
///
/// # Example
///
/// ```rust,ignore
/// pub struct MotionReg(pub &'static MotionDef);
/// inventory::collect!(MotionReg);
///
/// impl RegistryReg<MotionDef> for MotionReg {
///     fn def(&self) -> &'static MotionDef { self.0 }
/// }
/// ```
pub trait RegistryReg<T: RegistryEntry + 'static>: 'static {
	/// Returns the static definition reference from this wrapper.
	fn def(&self) -> &'static T;
}

/// Policy for handling duplicate keys during index construction.
#[derive(Clone, Copy, Debug, Default)]
pub enum DuplicatePolicy {
	/// Panic with detailed error message.
	///
	/// Best for debug builds and CI - fails fast and loud.
	#[default]
	Panic,
	/// Keep the first definition seen for a key.
	FirstWins,
	/// Overwrite with the last definition seen.
	LastWins,
}

impl DuplicatePolicy {
	/// Returns the appropriate policy based on build configuration.
	///
	/// - Debug builds: `Panic` for immediate feedback
	/// - Release builds: `FirstWins` for graceful degradation
	#[inline]
	pub fn for_build() -> Self {
		if cfg!(debug_assertions) {
			DuplicatePolicy::Panic
		} else {
			DuplicatePolicy::FirstWins
		}
	}
}

/// Indexed collection of registry definitions with O(1) lookup.
///
/// Built via [`RegistryBuilder`], provides:
/// - O(1) lookup by name, id, or alias via [`get`](Self::get)
/// - Sorted iteration via [`items`](Self::items)
/// - Length inspection via [`len`](Self::len) and [`is_empty`](Self::is_empty)
pub struct RegistryIndex<T: RegistryEntry + 'static> {
	items: Vec<&'static T>,
	by_key: HashMap<&'static str, &'static T>,
}

impl<T: RegistryEntry + 'static> RegistryIndex<T> {
	/// Looks up a definition by name, id, or alias.
	#[inline]
	pub fn get(&self, key: &str) -> Option<&'static T> {
		self.by_key.get(key).copied()
	}

	/// Returns all definitions in sorted order.
	#[inline]
	pub fn items(&self) -> &[&'static T] {
		&self.items
	}

	/// Returns the number of unique definitions (not keys).
	#[inline]
	pub fn len(&self) -> usize {
		self.items.len()
	}

	/// Returns true if the index contains no definitions.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.items.is_empty()
	}

	/// Returns an iterator over all definitions.
	#[inline]
	pub fn iter(&self) -> impl Iterator<Item = &'static T> + '_ {
		self.items.iter().copied()
	}
}

/// Builder for constructing a [`RegistryIndex`].
///
/// Collects definitions from inventory, applies sorting, validates for
/// duplicates, and produces the final index.
///
/// # Example
///
/// ```rust,ignore
/// let index = RegistryBuilder::new("motions")
///     .extend_inventory::<MotionReg>()
///     .sort_by(|a, b| a.name().cmp(b.name()))
///     .build();
/// ```
pub struct RegistryBuilder<T: RegistryEntry + 'static> {
	label: &'static str,
	defs: Vec<&'static T>,
	include_id: bool,
	include_name: bool,
	include_aliases: bool,
	policy: DuplicatePolicy,
}

impl<T: RegistryEntry + 'static> RegistryBuilder<T> {
	/// Creates a new builder with the given label for error messages.
	///
	/// Defaults:
	/// - `include_id`: true
	/// - `include_name`: true
	/// - `include_aliases`: true
	/// - `policy`: [`DuplicatePolicy::for_build()`]
	pub fn new(label: &'static str) -> Self {
		Self {
			label,
			defs: Vec::new(),
			include_id: true,
			include_name: true,
			include_aliases: true,
			policy: DuplicatePolicy::for_build(),
		}
	}

	/// Sets whether to index definitions by their id.
	pub fn include_id(mut self, on: bool) -> Self {
		self.include_id = on;
		self
	}

	/// Sets whether to index definitions by their name.
	pub fn include_name(mut self, on: bool) -> Self {
		self.include_name = on;
		self
	}

	/// Sets whether to index definitions by their aliases.
	pub fn include_aliases(mut self, on: bool) -> Self {
		self.include_aliases = on;
		self
	}

	/// Sets the duplicate key handling policy.
	pub fn duplicate_policy(mut self, policy: DuplicatePolicy) -> Self {
		self.policy = policy;
		self
	}

	/// Adds a single definition to the builder.
	pub fn push(mut self, def: &'static T) -> Self {
		self.defs.push(def);
		self
	}

	/// Adds multiple definitions to the builder.
	pub fn extend<I: IntoIterator<Item = &'static T>>(mut self, defs: I) -> Self {
		self.defs.extend(defs);
		self
	}

	/// Collects all definitions from inventory via the wrapper type.
	///
	/// The wrapper type `R` must implement [`RegistryReg<T>`] to expose
	/// the underlying definition. The wrapper must also be collected via
	/// `inventory::collect!(R)`.
	pub fn extend_inventory<R>(mut self) -> Self
	where
		R: RegistryReg<T>,
		inventory::iter<R>: IntoIterator<Item = &'static R>,
	{
		for reg in inventory::iter::<R> {
			self.defs.push(reg.def());
		}
		self
	}

	/// Sorts definitions using the provided comparison function.
	pub fn sort_by<F: FnMut(&&'static T, &&'static T) -> Ordering>(mut self, cmp: F) -> Self {
		self.defs.sort_by(cmp);
		self
	}

	/// Sorts definitions by priority (descending), then name, then id.
	///
	/// This is the default sort order for most registries.
	pub fn sort_default(mut self) -> Self {
		self.defs.sort_by(|a, b| {
			b.priority()
				.cmp(&a.priority())
				.then_with(|| a.name().cmp(b.name()))
				.then_with(|| a.id().cmp(b.id()))
		});
		self
	}

	/// Builds the index, validating for duplicates according to policy.
	///
	/// # Panics
	///
	/// Panics if duplicate keys are found and policy is [`DuplicatePolicy::Panic`].
	pub fn build(mut self) -> RegistryIndex<T> {
		let mut seen = std::collections::HashSet::with_capacity(self.defs.len());
		self.defs.retain(|d| seen.insert(*d as *const T as usize));

		let mut by_key = HashMap::with_capacity(self.defs.len() * 2);

		for &def in &self.defs {
			let meta = def.meta();
			if self.include_name {
				self.insert_key(&mut by_key, meta.name, def);
			}
			if self.include_id {
				self.insert_key(&mut by_key, meta.id, def);
			}
			if self.include_aliases {
				for &alias in meta.aliases {
					self.insert_key(&mut by_key, alias, def);
				}
			}
		}

		RegistryIndex {
			items: self.defs,
			by_key,
		}
	}

	fn insert_key(
		&self,
		map: &mut HashMap<&'static str, &'static T>,
		key: &'static str,
		def: &'static T,
	) {
		if let Some(&existing) = map.get(key) {
			if std::ptr::eq(existing, def) {
				return;
			}
			match self.policy {
				DuplicatePolicy::Panic => panic!(
					"duplicate registry key in {}: key={:?} existing_id={} new_id={}",
					self.label,
					key,
					existing.id(),
					def.id()
				),
				DuplicatePolicy::FirstWins => {}
				DuplicatePolicy::LastWins => {
					map.insert(key, def);
				}
			}
		} else {
			map.insert(key, def);
		}
	}
}

/// Registry wrapper for runtime-extensible registries.
///
/// Combines an immutable [`RegistryIndex`] of builtins with a mutable overlay
/// for runtime additions. Provides the same API as `RegistryIndex` plus
/// [`register`](Self::register) for adding definitions at runtime.
///
/// # Example
///
/// ```rust,ignore
/// static COMMANDS: LazyLock<RuntimeRegistry<CommandDef>> = LazyLock::new(|| {
///     let builtins = RegistryBuilder::new("commands")
///         .extend_inventory::<CommandReg>()
///         .sort_by(|a, b| a.name().cmp(b.name()))
///         .build();
///     RuntimeRegistry::new("commands", builtins)
/// });
///
/// // Later at runtime:
/// COMMANDS.register(&MY_PLUGIN_COMMAND);
/// ```
pub struct RuntimeRegistry<T: RegistryEntry + 'static> {
	label: &'static str,
	builtins: RegistryIndex<T>,
	extras_items: std::sync::RwLock<Vec<&'static T>>,
	extras_by_key: std::sync::RwLock<HashMap<&'static str, &'static T>>,
	policy: DuplicatePolicy,
}

impl<T: RegistryEntry + 'static> RuntimeRegistry<T> {
	/// Creates a new runtime registry with the given builtins.
	pub fn new(label: &'static str, builtins: RegistryIndex<T>) -> Self {
		Self {
			label,
			builtins,
			extras_items: std::sync::RwLock::new(Vec::new()),
			extras_by_key: std::sync::RwLock::new(HashMap::new()),
			policy: DuplicatePolicy::for_build(),
		}
	}

	/// Creates a new runtime registry with a custom duplicate policy.
	pub fn with_policy(
		label: &'static str,
		builtins: RegistryIndex<T>,
		policy: DuplicatePolicy,
	) -> Self {
		Self {
			label,
			builtins,
			extras_items: std::sync::RwLock::new(Vec::new()),
			extras_by_key: std::sync::RwLock::new(HashMap::new()),
			policy,
		}
	}

	/// Looks up a definition by name, id, or alias.
	///
	/// Checks runtime extras first (allowing overrides), then builtins.
	pub fn get(&self, key: &str) -> Option<&'static T> {
		self.extras_by_key
			.read()
			.expect("poisoned")
			.get(key)
			.copied()
			.or_else(|| self.builtins.get(key))
	}

	/// Registers a definition at runtime.
	///
	/// Returns `true` if the definition was added, `false` if it was already
	/// registered (either as builtin or previous runtime addition).
	///
	/// # Panics
	///
	/// Panics if the definition's keys conflict with existing keys and the
	/// policy is [`DuplicatePolicy::Panic`].
	pub fn register(&self, def: &'static T) -> bool {
		if self.builtins.items().iter().any(|&b| std::ptr::eq(b, def)) {
			return false;
		}

		let mut extras = self.extras_items.write().expect("poisoned");
		if extras.iter().any(|&e| std::ptr::eq(e, def)) {
			return false;
		}
		extras.push(def);
		drop(extras);

		let mut by_key = self.extras_by_key.write().expect("poisoned");
		let meta = def.meta();
		self.insert_key(&mut by_key, meta.name, def);
		self.insert_key(&mut by_key, meta.id, def);
		for &alias in meta.aliases {
			self.insert_key(&mut by_key, alias, def);
		}

		true
	}

	fn insert_key(
		&self,
		map: &mut HashMap<&'static str, &'static T>,
		key: &'static str,
		def: &'static T,
	) {
		if let Some(existing) = self.builtins.get(key) {
			if std::ptr::eq(existing, def) {
				return;
			}
			match self.policy {
				DuplicatePolicy::Panic => panic!(
					"runtime registry key conflict in {}: key={:?} conflicts with builtin",
					self.label, key
				),
				DuplicatePolicy::FirstWins => return,
				DuplicatePolicy::LastWins => {}
			}
		}

		if let Some(&existing) = map.get(key) {
			if std::ptr::eq(existing, def) {
				return;
			}
			match self.policy {
				DuplicatePolicy::Panic => {
					panic!("runtime registry key conflict in {}: key={:?}", self.label, key)
				}
				DuplicatePolicy::FirstWins => return,
				DuplicatePolicy::LastWins => {}
			}
		}

		map.insert(key, def);
	}

	/// Returns the number of unique definitions (builtins + extras).
	pub fn len(&self) -> usize {
		self.builtins.len() + self.extras_items.read().expect("poisoned").len()
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.builtins.is_empty() && self.extras_items.read().expect("poisoned").is_empty()
	}

	/// Returns all definitions (builtins followed by extras).
	///
	/// Note: The returned vector is a snapshot; runtime additions after this
	/// call won't be reflected.
	pub fn all(&self) -> Vec<&'static T> {
		let mut items: Vec<_> = self.builtins.items().to_vec();
		items.extend(self.extras_items.read().expect("poisoned").iter().copied());
		items
	}

	/// Returns the underlying builtins index.
	pub fn builtins(&self) -> &RegistryIndex<T> {
		&self.builtins
	}
}

/// Builds a secondary index map with custom keys.
///
/// Use for registries needing non-string indexes (trigger characters, numeric IDs).
pub fn build_map<T, K, F>(
	label: &'static str,
	items: &[&'static T],
	policy: DuplicatePolicy,
	mut key_of: F,
) -> HashMap<K, &'static T>
where
	T: 'static,
	K: Eq + std::hash::Hash + std::fmt::Debug,
	F: FnMut(&'static T) -> Option<K>,
{
	let mut map: HashMap<K, &'static T> = HashMap::with_capacity(items.len());

	for &item in items {
		let Some(key) = key_of(item) else { continue };

		if let Some(&existing) = map.get(&key) {
			if std::ptr::eq(existing, item) {
				continue;
			}
			match policy {
				DuplicatePolicy::Panic => {
					panic!("duplicate secondary key in {}: key={:?}", label, key)
				}
				DuplicatePolicy::FirstWins => {}
				DuplicatePolicy::LastWins => {
					map.insert(key, item);
				}
			}
		} else {
			map.insert(key, item);
		}
	}

	map
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{RegistryMeta, RegistrySource};

	/// Test definition type.
	struct TestDef {
		meta: RegistryMeta,
	}

	impl RegistryEntry for TestDef {
		fn meta(&self) -> &RegistryMeta {
			&self.meta
		}
	}

	static DEF_A: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::a",
			name: "a",
			aliases: &["alpha"],
			description: "Test A",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	static DEF_B: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::b",
			name: "b",
			aliases: &[],
			description: "Test B",
			priority: 10,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	#[test]
	fn test_index_lookup() {
		let index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_B)
			.duplicate_policy(DuplicatePolicy::Panic)
			.build();

		assert_eq!(index.len(), 2);

		// Lookup by name
		assert!(std::ptr::eq(index.get("a").unwrap(), &DEF_A));
		assert!(std::ptr::eq(index.get("b").unwrap(), &DEF_B));

		// Lookup by id
		assert!(std::ptr::eq(index.get("test::a").unwrap(), &DEF_A));

		// Lookup by alias
		assert!(std::ptr::eq(index.get("alpha").unwrap(), &DEF_A));

		// Not found
		assert!(index.get("unknown").is_none());
	}

	#[test]
	fn test_sort_default() {
		let index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_B)
			.sort_default()
			.build();

		// DEF_B has higher priority (10), so it comes first.
		assert!(std::ptr::eq(index.items()[0], &DEF_B));
		assert!(std::ptr::eq(index.items()[1], &DEF_A));
	}

	#[test]
	fn test_first_wins() {
		static DEF_A2: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::a2",
				name: "a", // Same name as DEF_A
				aliases: &[],
				description: "Test A2",
				priority: 0,
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
		};

		let index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_A2)
			.duplicate_policy(DuplicatePolicy::FirstWins)
			.build();

		// First wins: DEF_A should be in the index for key "a".
		assert!(std::ptr::eq(index.get("a").unwrap(), &DEF_A));
		// But DEF_A2 is still in items.
		assert_eq!(index.len(), 2);
	}

	#[test]
	fn test_last_wins() {
		static DEF_A2: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::a2",
				name: "a",
				aliases: &[],
				description: "Test A2",
				priority: 0,
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
		};

		let index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_A2)
			.duplicate_policy(DuplicatePolicy::LastWins)
			.build();

		// Last wins: DEF_A2 should be in the index for key "a".
		assert!(std::ptr::eq(index.get("a").unwrap(), &DEF_A2));
	}

	#[test]
	#[should_panic(expected = "duplicate registry key")]
	fn test_panic_on_duplicate() {
		static DEF_A2: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::a2",
				name: "a",
				aliases: &[],
				description: "Test A2",
				priority: 0,
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
		};

		let _index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_A2)
			.duplicate_policy(DuplicatePolicy::Panic)
			.build();
	}
}
