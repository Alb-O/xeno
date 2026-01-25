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
//!
//! # Invariants
//!
//! The registry enforces these hard invariants regardless of [`DuplicatePolicy`]:
//!
//! - **IDs are sacred**: Two definitions with the same `meta.id` is always fatal.
//! - **No ID shadowing**: A name or alias that equals any existing ID is always fatal.
//! - **ID-first lookup**: [`RegistryIndex::get`] checks `by_id` before `by_key`.
//!
//! Collisions between names/aliases (not involving IDs) are handled per policy.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::RegistryEntry;

/// Distinguishes the type of key being inserted.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum KeyKind {
	/// The definition's unique identifier (`meta.id`).
	Id,
	/// The definition's human-readable name (`meta.name`).
	Name,
	/// An alternative lookup name (`meta.aliases`).
	Alias,
}

impl std::fmt::Display for KeyKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			KeyKind::Id => write!(f, "id"),
			KeyKind::Name => write!(f, "name"),
			KeyKind::Alias => write!(f, "alias"),
		}
	}
}

/// Result of a successful key insertion.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InsertAction {
	/// Key was new; definition inserted.
	InsertedNew,
	/// Key existed; kept the existing definition (policy chose existing).
	KeptExisting,
	/// Key existed; replaced with new definition (policy chose new).
	ReplacedExisting,
}

/// Fatal insertion errors (always panic, regardless of policy).
#[derive(Debug, Clone)]
pub enum InsertFatal {
	/// Two definitions have the same `meta.id`.
	DuplicateId {
		key: &'static str,
		existing_id: &'static str,
		new_id: &'static str,
	},
	/// A name or alias shadows an existing ID.
	KeyShadowsId {
		kind: KeyKind,
		key: &'static str,
		id_owner: &'static str,
		new_id: &'static str,
	},
}

impl std::fmt::Display for InsertFatal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			InsertFatal::DuplicateId {
				key,
				existing_id,
				new_id,
			} => {
				write!(
					f,
					"duplicate ID: key={key:?} existing={existing_id} new={new_id}"
				)
			}
			InsertFatal::KeyShadowsId {
				kind,
				key,
				id_owner,
				new_id,
			} => {
				write!(
					f,
					"{kind} shadows ID: key={key:?} id_owner={id_owner} from={new_id}"
				)
			}
		}
	}
}

impl std::error::Error for InsertFatal {}

/// Records a non-fatal collision (name/alias conflicts resolved by policy).
#[derive(Debug, Clone)]
pub struct Collision {
	/// What kind of key collided.
	pub kind: KeyKind,
	/// The colliding key string.
	pub key: &'static str,
	/// The ID of the definition that already held this key.
	pub existing_id: &'static str,
	/// The ID of the new definition trying to claim this key.
	pub new_id: &'static str,
	/// Which definition won.
	pub winner_id: &'static str,
	/// What action was taken.
	pub action: InsertAction,
	/// The registry label where this collision occurred.
	pub registry: &'static str,
}

/// Winner selection function: returns `true` if `new` should replace `existing`.
pub type ChooseWinner<T> =
	fn(kind: KeyKind, key: &'static str, existing: &'static T, new: &'static T) -> bool;

/// Abstraction over key storage for shared insertion logic.
///
/// Implemented by both build-time and runtime stores to share [`insert_typed_key`].
pub trait KeyStore<T: RegistryEntry + 'static> {
	/// Returns the definition that owns this string as an ID, if any.
	fn get_id_owner(&self, id: &str) -> Option<&'static T>;

	/// Returns the current winner in the name/alias namespace.
	fn get_key_winner(&self, key: &str) -> Option<&'static T>;

	/// Sets the winner in the name/alias namespace.
	fn set_key_winner(&mut self, key: &'static str, def: &'static T);

	/// Inserts into the ID table. Returns the previous occupant if any.
	fn insert_id(&mut self, id: &'static str, def: &'static T) -> Option<&'static T>;

	/// Records a collision for diagnostics.
	fn push_collision(&mut self, c: Collision);
}

/// Inserts a key with proper invariant checking.
///
/// This is the single authoritative insertion routine used by both build-time
/// and runtime registration paths. It enforces:
///
/// - ID uniqueness (fatal error on duplicate)
/// - No name/alias shadowing IDs (fatal error)
/// - Policy-based winner selection for name/alias conflicts
/// - Collision recording for diagnostics
///
/// # Errors
///
/// Returns `Err(InsertFatal)` for invariant violations that are always fatal
/// regardless of duplicate policy.
pub fn insert_typed_key<T: RegistryEntry + 'static>(
	store: &mut dyn KeyStore<T>,
	registry_label: &'static str,
	choose_winner: ChooseWinner<T>,
	kind: KeyKind,
	key: &'static str,
	def: &'static T,
) -> Result<InsertAction, InsertFatal> {
	match kind {
		KeyKind::Id => {
			if let Some(prev) = store.insert_id(key, def) {
				if !std::ptr::eq(prev, def) {
					return Err(InsertFatal::DuplicateId {
						key,
						existing_id: prev.id(),
						new_id: def.id(),
					});
				}
			}
			Ok(InsertAction::InsertedNew)
		}
		KeyKind::Name | KeyKind::Alias => {
			if let Some(id_owner) = store.get_id_owner(key) {
				if !std::ptr::eq(id_owner, def) {
					return Err(InsertFatal::KeyShadowsId {
						kind,
						key,
						id_owner: id_owner.id(),
						new_id: def.id(),
					});
				}
				return Ok(InsertAction::KeptExisting);
			}

			if let Some(existing) = store.get_key_winner(key) {
				if std::ptr::eq(existing, def) {
					return Ok(InsertAction::KeptExisting);
				}

				let new_wins = choose_winner(kind, key, existing, def);
				let (action, winner_id) = if new_wins {
					store.set_key_winner(key, def);
					(InsertAction::ReplacedExisting, def.id())
				} else {
					(InsertAction::KeptExisting, existing.id())
				};

				store.push_collision(Collision {
					kind,
					key,
					existing_id: existing.id(),
					new_id: def.id(),
					winner_id,
					action,
					registry: registry_label,
				});

				Ok(action)
			} else {
				store.set_key_winner(key, def);
				Ok(InsertAction::InsertedNew)
			}
		}
	}
}

/// Trait for registry wrapper types to expose their definition.
///
/// Implement this for your registry's wrapper type (e.g., `MotionReg`)
/// to allow [`RegistryBuilder::extend_inventory`] to extract definitions.
pub trait RegistryReg<T: RegistryEntry + 'static>: 'static {
	/// Returns the static definition reference from this wrapper.
	fn def(&self) -> &'static T;
}

/// Policy for handling duplicate keys during index construction.
///
/// # Interaction with Sort Order
///
/// When using [`RegistryBuilder::sort_default`] (priority descending):
///
/// - [`FirstWins`](Self::FirstWins) → highest priority wins (intended behavior)
/// - [`LastWins`](Self::LastWins) → lowest priority wins (usually wrong)
///
/// The default from [`for_build()`](Self::for_build) returns `FirstWins` in release
/// and `Panic` in debug, matching the intended semantics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DuplicatePolicy {
	/// Panic with detailed error message.
	///
	/// Best for debug builds and CI - fails fast and loud.
	#[default]
	Panic,
	/// Keep the first definition seen for a key.
	///
	/// With default priority sorting, this means highest-priority wins.
	FirstWins,
	/// Overwrite with the last definition seen.
	///
	/// With default priority sorting, this means lowest-priority wins
	/// (usually not what you want).
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
/// - O(1) ID-first lookup via [`get`](Self::get)
/// - All definitions (including shadowed) via [`items_all`](Self::items_all)
/// - Effective definitions (winners only) via [`items`](Self::items)
/// - Collision diagnostics via [`collisions`](Self::collisions)
///
/// # Lookup Semantics
///
/// The [`get`](Self::get) method uses ID-first lookup:
/// 1. Check `by_id` for exact ID match
/// 2. Fall back to `by_key` for name/alias lookup
///
/// This ensures stable references: `get("some-id")` always resolves via the ID
/// namespace, never accidentally matching a name or alias.
pub struct RegistryIndex<T: RegistryEntry + 'static> {
	/// ID → definition (sacred, no collisions allowed).
	by_id: HashMap<&'static str, &'static T>,
	/// Name/Alias → winner definition (policy-resolved).
	by_key: HashMap<&'static str, &'static T>,
	/// All definitions submitted to the builder (including shadowed).
	items_all: Vec<&'static T>,
	/// Effective definitions: unique set reachable via indices.
	items_effective: Vec<&'static T>,
	/// Recorded collisions for diagnostics.
	collisions: Vec<Collision>,
}

impl<T: RegistryEntry + 'static> RegistryIndex<T> {
	/// Looks up a definition by ID, name, or alias.
	///
	/// Uses ID-first lookup: checks `by_id` before `by_key`. This ensures that
	/// looking up an ID always returns that specific definition, even if another
	/// definition has a name or alias that happens to match.
	#[inline]
	pub fn get(&self, key: &str) -> Option<&'static T> {
		self.by_id
			.get(key)
			.copied()
			.or_else(|| self.by_key.get(key).copied())
	}

	/// Returns the definition for a given ID, if it exists.
	#[inline]
	pub fn get_by_id(&self, id: &str) -> Option<&'static T> {
		self.by_id.get(id).copied()
	}

	/// Returns all definitions submitted to the builder (including shadowed).
	///
	/// Use this for iteration that needs to see every definition, e.g., for
	/// help text or diagnostics that should list shadowed items.
	#[inline]
	pub fn items_all(&self) -> &[&'static T] {
		&self.items_all
	}

	/// Returns effective definitions: unique set reachable via indices.
	///
	/// This is the preferred iteration method for most use cases. Shadowed
	/// definitions are excluded.
	#[inline]
	pub fn items(&self) -> &[&'static T] {
		&self.items_effective
	}

	/// Returns recorded collisions for diagnostics.
	#[inline]
	pub fn collisions(&self) -> &[Collision] {
		&self.collisions
	}

	/// Returns the number of effective definitions (not keys, not shadowed).
	#[inline]
	pub fn len(&self) -> usize {
		self.items_effective.len()
	}

	/// Returns true if the index contains no definitions.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.items_effective.is_empty()
	}

	/// Returns an iterator over effective definitions.
	#[inline]
	pub fn iter(&self) -> impl Iterator<Item = &'static T> + '_ {
		self.items_effective.iter().copied()
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

	/// Builds the index with two-pass insertion and invariant enforcement.
	///
	/// Pass A inserts all IDs first via [`insert_typed_key`] with `kind=Id`, which
	/// is fatal on duplicate. Pass B inserts names/aliases, which is fatal if any
	/// key shadows an existing ID but otherwise records collisions per policy.
	/// Finally, `items_effective` is computed as the unique set reachable via indices.
	///
	/// # Panics
	///
	/// - Always panics on duplicate IDs (regardless of policy)
	/// - Always panics if a name/alias shadows an existing ID
	/// - Panics on name/alias collisions only if policy is [`DuplicatePolicy::Panic`]
	pub fn build(mut self) -> RegistryIndex<T> {
		let mut seen: std::collections::HashSet<*const T> =
			std::collections::HashSet::with_capacity(self.defs.len());
		self.defs.retain(|d| seen.insert(*d as *const T));

		let mut store = BuildStore::<T> {
			by_id: HashMap::with_capacity(self.defs.len()),
			by_key: HashMap::with_capacity(self.defs.len() * 2),
			collisions: Vec::new(),
		};

		let choose_winner = self.make_choose_winner();

		if self.include_id {
			for &def in &self.defs {
				if let Err(e) = insert_typed_key(
					&mut store,
					self.label,
					choose_winner,
					KeyKind::Id,
					def.meta().id,
					def,
				) {
					panic!("registry {}: {}", self.label, e);
				}
			}
		}

		for &def in &self.defs {
			let meta = def.meta();

			if self.include_name {
				if let Err(e) = insert_typed_key(
					&mut store,
					self.label,
					choose_winner,
					KeyKind::Name,
					meta.name,
					def,
				) {
					panic!("registry {}: {}", self.label, e);
				}
			}

			if self.include_aliases {
				for &alias in meta.aliases {
					if let Err(e) = insert_typed_key(
						&mut store,
						self.label,
						choose_winner,
						KeyKind::Alias,
						alias,
						def,
					) {
						panic!("registry {}: {}", self.label, e);
					}
				}
			}
		}

		let mut effective_set: std::collections::HashSet<*const T> =
			std::collections::HashSet::with_capacity(self.defs.len());
		for &def in store.by_id.values() {
			effective_set.insert(def as *const T);
		}
		for &def in store.by_key.values() {
			effective_set.insert(def as *const T);
		}

		let items_effective: Vec<&'static T> = self
			.defs
			.iter()
			.copied()
			.filter(|&d| effective_set.contains(&(d as *const T)))
			.collect();

		RegistryIndex {
			by_id: store.by_id,
			by_key: store.by_key,
			items_all: self.defs,
			items_effective,
			collisions: store.collisions,
		}
	}

	/// Creates a winner selection function based on the policy.
	fn make_choose_winner(&self) -> ChooseWinner<T> {
		match self.policy {
			DuplicatePolicy::Panic => |kind, key, existing, new| {
				panic!(
					"duplicate registry key: kind={} key={:?} existing_id={} new_id={}",
					kind,
					key,
					existing.id(),
					new.id()
				);
			},
			DuplicatePolicy::FirstWins => |_, _, _, _| false,
			DuplicatePolicy::LastWins => |_, _, _, _| true,
		}
	}
}

/// Temporary storage for build-time key insertion.
struct BuildStore<T: RegistryEntry + 'static> {
	by_id: HashMap<&'static str, &'static T>,
	by_key: HashMap<&'static str, &'static T>,
	collisions: Vec<Collision>,
}

impl<T: RegistryEntry + 'static> KeyStore<T> for BuildStore<T> {
	fn get_id_owner(&self, id: &str) -> Option<&'static T> {
		self.by_id.get(id).copied()
	}

	fn get_key_winner(&self, key: &str) -> Option<&'static T> {
		self.by_key.get(key).copied()
	}

	fn set_key_winner(&mut self, key: &'static str, def: &'static T) {
		self.by_key.insert(key, def);
	}

	fn insert_id(&mut self, id: &'static str, def: &'static T) -> Option<&'static T> {
		self.by_id.insert(id, def)
	}

	fn push_collision(&mut self, c: Collision) {
		self.collisions.push(c);
	}
}

/// Runtime overlay for registry extensions.
///
/// Holds definitions added at runtime, with their own ID and key namespaces.
struct RuntimeExtras<T: RegistryEntry + 'static> {
	/// Definitions added at runtime.
	items: Vec<&'static T>,
	/// ID → definition (sacred namespace for extras).
	by_id: HashMap<&'static str, &'static T>,
	/// Name/Alias → winner definition (policy-resolved).
	by_key: HashMap<&'static str, &'static T>,
	/// Recorded collisions for diagnostics.
	collisions: Vec<Collision>,
}

impl<T: RegistryEntry + 'static> Default for RuntimeExtras<T> {
	fn default() -> Self {
		Self {
			items: Vec::new(),
			by_id: HashMap::new(),
			by_key: HashMap::new(),
			collisions: Vec::new(),
		}
	}
}

/// Registry wrapper for runtime-extensible registries.
///
/// Combines an immutable [`RegistryIndex`] of builtins with a mutable overlay
/// for runtime additions. Provides the same API as `RegistryIndex` plus
/// [`register`](Self::register) for adding definitions at runtime.
///
/// Registration is atomic: either the definition and all its keys are added,
/// or none are (on conflict with `DuplicatePolicy::Panic`).
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
///
/// # Performance Considerations
///
/// The current implementation uses `RwLock<RuntimeExtras<T>>` which takes a read
/// lock on every [`get()`](Self::get) call. This is acceptable for current usage
/// but has upgrade paths if `get()` becomes hot:
///
/// ## Freeze-After-Init Pattern
///
/// If runtime registration only happens during startup/plugin-load:
///
/// ```ignore
/// impl<T> RuntimeRegistry<T> {
///     /// Call after all plugins loaded. Merges extras into builtins for lock-free reads.
///     pub fn freeze(&self) { ... }
/// }
/// ```
///
/// ## Snapshot Swapping (ArcSwap)
///
/// If registration can happen at any time but is rare, use `arc_swap::ArcSwap`
/// for lock-free reads with clone-and-swap writes.
///
/// # Static Lifetime Constraint
///
/// Requires `&'static T` and `&'static str` keys. This works for:
///
/// - Compile-time builtins
/// - Crates linked at startup (inventory pattern)
/// - Leaked allocations (acceptable for long-lived plugins)
///
/// Does **not** work for truly dynamic plugins (dlopen, WASM, user scripts)
/// or reloadable/unloadable plugins. See "Layered Registry" below.
///
/// # Future: Layered Registry for Unloadable Plugins
///
/// If unloadable plugins are needed, consider layered scopes:
///
/// ```ignore
/// pub struct LayeredRegistry<T> {
///     builtins: RegistryIndex<T>,
///     layers: Vec<(PluginId, HashMap<String, Arc<T>>)>,
/// }
///
/// impl<T> LayeredRegistry<T> {
///     /// Lookup: newest layer -> older layers -> builtins
///     pub fn get(&self, key: &str) -> Option<&T> { ... }
///     /// Remove entire plugin layer
///     pub fn unload_plugin(&mut self, id: PluginId) { ... }
/// }
/// ```
///
/// Benefits: `Arc<T>` + `String` keys (no 'static), deterministic unload,
/// natural shadowing (newer layers win).
pub struct RuntimeRegistry<T: RegistryEntry + 'static> {
	label: &'static str,
	builtins: RegistryIndex<T>,
	extras: std::sync::RwLock<RuntimeExtras<T>>,
	policy: DuplicatePolicy,
}

impl<T: RegistryEntry + 'static> RuntimeRegistry<T> {
	/// Creates a new runtime registry with the given builtins.
	pub fn new(label: &'static str, builtins: RegistryIndex<T>) -> Self {
		Self {
			label,
			builtins,
			extras: std::sync::RwLock::new(RuntimeExtras::default()),
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
			extras: std::sync::RwLock::new(RuntimeExtras::default()),
			policy,
		}
	}

	/// Looks up a definition by ID, name, or alias.
	///
	/// Uses ID-first, extras-first lookup: `extras.by_id` → `builtins.by_id` →
	/// `extras.by_key` → `builtins.by_key`.
	pub fn get(&self, key: &str) -> Option<&'static T> {
		let extras = self.extras.read().expect("poisoned");

		extras
			.by_id
			.get(key)
			.copied()
			.or_else(|| self.builtins.get_by_id(key))
			.or_else(|| extras.by_key.get(key).copied())
			.or_else(|| self.builtins.get(key))
	}

	/// Returns the definition for a given ID, if it exists.
	pub fn get_by_id(&self, id: &str) -> Option<&'static T> {
		let extras = self.extras.read().expect("poisoned");
		extras
			.by_id
			.get(id)
			.copied()
			.or_else(|| self.builtins.get_by_id(id))
	}

	/// Registers a definition at runtime.
	///
	/// Returns `true` if added, `false` if already registered as builtin or extra.
	///
	/// # Panics
	///
	/// - Always panics on duplicate IDs (regardless of policy)
	/// - Always panics if a name/alias shadows an existing ID
	/// - Panics on name/alias collisions only if policy is [`DuplicatePolicy::Panic`]
	pub fn register(&self, def: &'static T) -> bool {
		if self
			.builtins
			.items_all()
			.iter()
			.any(|&b| std::ptr::eq(b, def))
		{
			return false;
		}

		let mut extras = self.extras.write().expect("poisoned");

		if extras.items.iter().any(|&e| std::ptr::eq(e, def)) {
			return false;
		}

		let meta = def.meta();
		let choose_winner = self.make_choose_winner();
		let mut store = RuntimeStore {
			builtins: &self.builtins,
			extras: &mut extras,
		};

		if let Err(e) = insert_typed_key(
			&mut store,
			self.label,
			choose_winner,
			KeyKind::Id,
			meta.id,
			def,
		) {
			panic!("runtime registry {}: {}", self.label, e);
		}

		if let Err(e) = insert_typed_key(
			&mut store,
			self.label,
			choose_winner,
			KeyKind::Name,
			meta.name,
			def,
		) {
			panic!("runtime registry {}: {}", self.label, e);
		}

		for &alias in meta.aliases {
			if let Err(e) = insert_typed_key(
				&mut store,
				self.label,
				choose_winner,
				KeyKind::Alias,
				alias,
				def,
			) {
				panic!("runtime registry {}: {}", self.label, e);
			}
		}

		extras.items.push(def);
		true
	}

	fn make_choose_winner(&self) -> ChooseWinner<T> {
		match self.policy {
			DuplicatePolicy::Panic => |kind, key, existing, new| {
				panic!(
					"runtime registry key conflict: kind={} key={:?} existing_id={} new_id={}",
					kind,
					key,
					existing.id(),
					new.id()
				);
			},
			DuplicatePolicy::FirstWins => |_, _, _, _| false,
			DuplicatePolicy::LastWins => |_, _, _, _| true,
		}
	}

	/// Returns the number of unique definitions (builtins + extras).
	pub fn len(&self) -> usize {
		self.builtins.len() + self.extras.read().expect("poisoned").items.len()
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.builtins.is_empty() && self.extras.read().expect("poisoned").items.is_empty()
	}

	/// Returns all definitions (builtins followed by extras).
	///
	/// Note: The returned vector is a snapshot; runtime additions after this
	/// call won't be reflected.
	pub fn all(&self) -> Vec<&'static T> {
		let mut items: Vec<_> = self.builtins.items().to_vec();
		items.extend(self.extras.read().expect("poisoned").items.iter().copied());
		items
	}

	/// Returns the underlying builtins index.
	pub fn builtins(&self) -> &RegistryIndex<T> {
		&self.builtins
	}

	/// Returns an iterator over builtin definitions only.
	///
	/// For all definitions including runtime additions, use [`all`](Self::all).
	pub fn iter(&self) -> impl Iterator<Item = &'static T> + '_ {
		self.builtins.iter()
	}

	/// Returns the builtin items slice.
	pub fn items(&self) -> &[&'static T] {
		self.builtins.items()
	}

	/// Returns all recorded collisions (builtins + runtime).
	pub fn collisions(&self) -> Vec<Collision> {
		let mut collisions = self.builtins.collisions().to_vec();
		collisions.extend(self.extras.read().expect("poisoned").collisions.iter().cloned());
		collisions
	}
}

/// Layered [`KeyStore`] for runtime insertion: checks builtins first, then extras.
struct RuntimeStore<'a, T: RegistryEntry + 'static> {
	builtins: &'a RegistryIndex<T>,
	extras: &'a mut RuntimeExtras<T>,
}

impl<T: RegistryEntry + 'static> KeyStore<T> for RuntimeStore<'_, T> {
	fn get_id_owner(&self, id: &str) -> Option<&'static T> {
		self.builtins
			.get_by_id(id)
			.or_else(|| self.extras.by_id.get(id).copied())
	}

	fn get_key_winner(&self, key: &str) -> Option<&'static T> {
		self.extras
			.by_key
			.get(key)
			.copied()
			.or_else(|| self.builtins.get(key))
	}

	fn set_key_winner(&mut self, key: &'static str, def: &'static T) {
		self.extras.by_key.insert(key, def);
	}

	fn insert_id(&mut self, id: &'static str, def: &'static T) -> Option<&'static T> {
		self.builtins
			.get_by_id(id)
			.or_else(|| self.extras.by_id.insert(id, def))
	}

	fn push_collision(&mut self, c: Collision) {
		self.extras.collisions.push(c);
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

	#[test]
	#[should_panic(expected = "duplicate ID")]
	fn test_duplicate_id_fatal_regardless_of_policy() {
		// Two definitions with same ID should always be fatal, even with FirstWins
		static DEF_DUP_ID: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::a", // Same ID as DEF_A
				name: "different_name",
				aliases: &[],
				description: "Duplicate ID",
				priority: 0,
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
		};

		let _index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_DUP_ID)
			.duplicate_policy(DuplicatePolicy::FirstWins) // Not Panic!
			.build();
	}

	#[test]
	#[should_panic(expected = "shadows ID")]
	fn test_name_shadows_id_fatal() {
		// Name that equals another definition's ID should be fatal
		static DEF_NAME_SHADOWS: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::shadow",
				name: "test::a", // Name equals DEF_A's ID
				aliases: &[],
				description: "Name shadows ID",
				priority: 0,
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
		};

		let _index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_NAME_SHADOWS)
			.duplicate_policy(DuplicatePolicy::FirstWins) // Not Panic!
			.build();
	}

	#[test]
	#[should_panic(expected = "shadows ID")]
	fn test_alias_shadows_id_fatal() {
		// Alias that equals another definition's ID should be fatal
		static DEF_ALIAS_SHADOWS: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::shadow2",
				name: "shadow2",
				aliases: &["test::a"], // Alias equals DEF_A's ID
				description: "Alias shadows ID",
				priority: 0,
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
		};

		let _index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_ALIAS_SHADOWS)
			.duplicate_policy(DuplicatePolicy::FirstWins) // Not Panic!
			.build();
	}

	#[test]
	fn test_collision_recorded() {
		// Name collision should be recorded (not fatal with FirstWins)
		static DEF_COLLISION: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::collision",
				name: "a", // Same name as DEF_A
				aliases: &[],
				description: "Collision test",
				priority: 0,
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
		};

		let index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_COLLISION)
			.duplicate_policy(DuplicatePolicy::FirstWins)
			.build();

		// Should have recorded the collision
		assert_eq!(index.collisions().len(), 1);
		let collision = &index.collisions()[0];
		assert_eq!(collision.kind, KeyKind::Name);
		assert_eq!(collision.key, "a");
		assert_eq!(collision.existing_id, "test::a");
		assert_eq!(collision.new_id, "test::collision");
		assert_eq!(collision.winner_id, "test::a"); // FirstWins
	}

	#[test]
	fn test_id_first_lookup() {
		// ID lookup should take precedence over name/alias
		static DEF_ID_FIRST: TestDef = TestDef {
			meta: RegistryMeta {
				id: "lookup_target", // This is the ID
				name: "different_name",
				aliases: &[],
				description: "ID first test",
				priority: 0,
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
		};

		let index = RegistryBuilder::new("test")
			.push(&DEF_ID_FIRST)
			.build();

		// Lookup by ID should work
		assert!(std::ptr::eq(index.get("lookup_target").unwrap(), &DEF_ID_FIRST));
		assert!(std::ptr::eq(
			index.get_by_id("lookup_target").unwrap(),
			&DEF_ID_FIRST
		));
	}

	#[test]
	fn test_items_all_vs_effective() {
		// items_all includes shadowed, items (effective) excludes shadowed
		static DEF_SHADOWED: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::shadowed",
				name: "a", // Same name as DEF_A
				aliases: &[],
				description: "Shadowed def",
				priority: -1, // Lower priority
				source: RegistrySource::Builtin,
				required_caps: &[],
				flags: 0,
			},
		};

		let index = RegistryBuilder::new("test")
			.push(&DEF_A)
			.push(&DEF_SHADOWED)
			.sort_default() // DEF_A (priority 0) before DEF_SHADOWED (priority -1)
			.duplicate_policy(DuplicatePolicy::FirstWins)
			.build();

		// items_all contains both
		assert_eq!(index.items_all().len(), 2);

		// items (effective) contains both because both have unique IDs
		// and are therefore reachable via by_id
		assert_eq!(index.items().len(), 2);

		// But lookup by name "a" returns DEF_A (first wins)
		assert!(std::ptr::eq(index.get("a").unwrap(), &DEF_A));
	}

	#[test]
	#[should_panic(expected = "duplicate ID")]
	fn test_runtime_duplicate_id_with_builtin_fatal() {
		// Runtime def with same ID as builtin should be fatal
		static DEF_RUNTIME_DUP: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::a", // Same ID as DEF_A
				name: "runtime_name",
				aliases: &[],
				description: "Runtime duplicate ID",
				priority: 0,
				source: RegistrySource::Runtime,
				required_caps: &[],
				flags: 0,
			},
		};

		let builtins = RegistryBuilder::new("test")
			.push(&DEF_A)
			.duplicate_policy(DuplicatePolicy::FirstWins)
			.build();

		let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
		registry.register(&DEF_RUNTIME_DUP);
	}

	#[test]
	#[should_panic(expected = "shadows ID")]
	fn test_runtime_name_shadows_builtin_id_fatal() {
		// Runtime name that equals builtin ID should be fatal
		static DEF_RUNTIME_SHADOW: TestDef = TestDef {
			meta: RegistryMeta {
				id: "test::runtime_shadow",
				name: "test::a", // Name equals builtin ID
				aliases: &[],
				description: "Runtime name shadows builtin ID",
				priority: 0,
				source: RegistrySource::Runtime,
				required_caps: &[],
				flags: 0,
			},
		};

		let builtins = RegistryBuilder::new("test")
			.push(&DEF_A)
			.duplicate_policy(DuplicatePolicy::FirstWins)
			.build();

		let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
		registry.register(&DEF_RUNTIME_SHADOW);
	}

	#[test]
	fn test_runtime_id_first_lookup() {
		// Runtime registry should use ID-first lookup
		static DEF_RUNTIME: TestDef = TestDef {
			meta: RegistryMeta {
				id: "runtime::def",
				name: "runtime_name",
				aliases: &[],
				description: "Runtime def",
				priority: 0,
				source: RegistrySource::Runtime,
				required_caps: &[],
				flags: 0,
			},
		};

		let builtins = RegistryBuilder::new("test")
			.push(&DEF_A)
			.duplicate_policy(DuplicatePolicy::FirstWins)
			.build();

		let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
		registry.register(&DEF_RUNTIME);

		// Lookup by ID should work for both builtin and runtime
		assert!(std::ptr::eq(registry.get("test::a").unwrap(), &DEF_A));
		assert!(std::ptr::eq(registry.get("runtime::def").unwrap(), &DEF_RUNTIME));

		// get_by_id should also work
		assert!(std::ptr::eq(registry.get_by_id("test::a").unwrap(), &DEF_A));
		assert!(std::ptr::eq(
			registry.get_by_id("runtime::def").unwrap(),
			&DEF_RUNTIME
		));
	}

	#[test]
	#[should_panic(expected = "shadows ID")]
	fn test_runtime_name_shadows_runtime_id_fatal() {
		static DEF_RUNTIME1: TestDef = TestDef {
			meta: RegistryMeta {
				id: "runtime::first",
				name: "first_name",
				aliases: &[],
				description: "Runtime def 1",
				priority: 0,
				source: RegistrySource::Runtime,
				required_caps: &[],
				flags: 0,
			},
		};

		static DEF_RUNTIME2: TestDef = TestDef {
			meta: RegistryMeta {
				id: "runtime::second",
				name: "runtime::first", // Name equals first runtime def's ID
				aliases: &[],
				description: "Runtime def 2",
				priority: 0,
				source: RegistrySource::Runtime,
				required_caps: &[],
				flags: 0,
			},
		};

		let builtins = RegistryBuilder::new("test")
			.duplicate_policy(DuplicatePolicy::FirstWins)
			.build();

		let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
		registry.register(&DEF_RUNTIME1);
		registry.register(&DEF_RUNTIME2);
	}

	#[test]
	fn test_runtime_collision_recorded() {
		static DEF_RUNTIME1: TestDef = TestDef {
			meta: RegistryMeta {
				id: "runtime::first",
				name: "shared_name",
				aliases: &[],
				description: "Runtime def 1",
				priority: 0,
				source: RegistrySource::Runtime,
				required_caps: &[],
				flags: 0,
			},
		};

		static DEF_RUNTIME2: TestDef = TestDef {
			meta: RegistryMeta {
				id: "runtime::second",
				name: "shared_name", // Same name, should record collision
				aliases: &[],
				description: "Runtime def 2",
				priority: 0,
				source: RegistrySource::Runtime,
				required_caps: &[],
				flags: 0,
			},
		};

		let builtins = RegistryBuilder::new("test")
			.duplicate_policy(DuplicatePolicy::FirstWins)
			.build();

		let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
		registry.register(&DEF_RUNTIME1);
		registry.register(&DEF_RUNTIME2);

		let collisions = registry.collisions();
		assert_eq!(collisions.len(), 1);
		assert_eq!(collisions[0].kind, KeyKind::Name);
		assert_eq!(collisions[0].key, "shared_name");
		assert_eq!(collisions[0].existing_id, "runtime::first");
		assert_eq!(collisions[0].new_id, "runtime::second");
		assert_eq!(collisions[0].winner_id, "runtime::first"); // FirstWins
	}
}
