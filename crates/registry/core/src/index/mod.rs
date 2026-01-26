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
#[derive(Debug, Clone, PartialEq, Eq)]
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
			if let Some(prev) = store.insert_id(key, def)
				&& !std::ptr::eq(prev, def)
			{
				return Err(InsertFatal::DuplicateId {
					key,
					existing_id: prev.id(),
					new_id: def.id(),
				});
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
	Panic,
	/// Keep the first definition seen for a key.
	FirstWins,
	/// Overwrite with the last definition seen.
	LastWins,
	/// Select winner by priority (higher wins), then source rank, then ID.
	///
	/// This provides a stable, deterministic total order for collision resolution.
	#[default]
	ByPriority,
}

impl DuplicatePolicy {
	/// Returns the appropriate policy based on build configuration.
	///
	/// - Debug builds: `Panic` for immediate feedback
	/// - Release builds: `ByPriority` for deterministic resolution
	#[inline]
	pub fn for_build() -> Self {
		if cfg!(debug_assertions) {
			DuplicatePolicy::Panic
		} else {
			DuplicatePolicy::ByPriority
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

impl<T: RegistryEntry + 'static> Clone for RegistryIndex<T> {
	fn clone(&self) -> Self {
		Self {
			by_id: self.by_id.clone(),
			by_key: self.by_key.clone(),
			items_all: self.items_all.clone(),
			items_effective: self.items_effective.clone(),
			collisions: self.collisions.clone(),
		}
	}
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

	/// Sorts definitions using the global total order.
	///
	/// 1. Priority (Descending)
	/// 2. Source Rank (Builtin > Crate > Runtime)
	/// 3. ID (Lexical higher wins)
	pub fn sort_default(mut self) -> Self {
		self.defs.sort_by(|a, b| b.total_order_cmp(a));
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

			if self.include_name
				&& let Err(e) = insert_typed_key(
					&mut store,
					self.label,
					choose_winner,
					KeyKind::Name,
					meta.name,
					def,
				) {
				panic!("registry {}: {}", self.label, e);
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
			DuplicatePolicy::ByPriority => {
				|_, _, existing, new| new.total_order_cmp(existing) == Ordering::Greater
			}
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
		match self.by_id.entry(id) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
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

impl<T: RegistryEntry + 'static> Clone for RuntimeExtras<T> {
	fn clone(&self) -> Self {
		Self {
			items: self.items.clone(),
			by_id: self.by_id.clone(),
			by_key: self.by_key.clone(),
			collisions: self.collisions.clone(),
		}
	}
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

macro_rules! poison_policy {
	($lock:expr, $method:ident) => {
		if cfg!(any(test, debug_assertions)) {
			$lock.$method().unwrap_or_else(|e| e.into_inner())
		} else {
			$lock.$method().expect("registry lock poisoned")
		}
	};
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
		let extras = poison_policy!(self.extras, read);

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
		let extras = poison_policy!(self.extras, read);
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

		let mut extras_guard = poison_policy!(self.extras, write);

		if extras_guard.items.iter().any(|&e| std::ptr::eq(e, def)) {
			return false;
		}

		let mut extras = (*extras_guard).clone();
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
		*extras_guard = extras;
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
			DuplicatePolicy::ByPriority => {
				|_, _, existing, new| new.total_order_cmp(existing) == Ordering::Greater
			}
		}
	}

	/// Returns the number of unique definitions (builtins + extras).
	pub fn len(&self) -> usize {
		self.builtins.len() + poison_policy!(self.extras, read).items.len()
	}

	/// Returns true if the registry contains no definitions.
	pub fn is_empty(&self) -> bool {
		self.builtins.is_empty() && poison_policy!(self.extras, read).items.is_empty()
	}

	/// Returns all definitions (builtins followed by extras).
	///
	/// Note: The returned vector is a snapshot; runtime additions after this
	/// call won't be reflected.
	pub fn all(&self) -> Vec<&'static T> {
		let mut items: Vec<_> = self.builtins.items().to_vec();
		items.extend(poison_policy!(self.extras, read).items.iter().copied());
		items
	}

	/// Returns definitions added at runtime.
	///
	/// Useful for secondary dispatch maps that need to selectively process
	/// runtime extensions without re-scanning the immutable builtin set.
	pub fn extras_items(&self) -> Vec<&'static T> {
		poison_policy!(self.extras, read).items.clone()
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
		collisions.extend(poison_policy!(self.extras, read).collisions.iter().cloned());
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
		if let Some(builtin) = self.builtins.get_by_id(id) {
			return Some(builtin);
		}
		match self.extras.by_id.entry(id) {
			std::collections::hash_map::Entry::Vacant(v) => {
				v.insert(def);
				None
			}
			std::collections::hash_map::Entry::Occupied(o) => Some(*o.get()),
		}
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
	T: RegistryEntry + 'static,
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
				DuplicatePolicy::ByPriority => {
					if item.total_order_cmp(existing) == Ordering::Greater {
						map.insert(key, item);
					}
				}
			}
		} else {
			map.insert(key, item);
		}
	}

	map
}

#[cfg(test)]
mod tests;
