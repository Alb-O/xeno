use std::sync::Arc;

use rustc_hash::FxHashMap;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(u32);

impl Symbol {
	pub const INVALID: Symbol = Symbol(u32::MAX);

	#[inline]
	pub fn is_valid(self) -> bool {
		self != Self::INVALID
	}

	#[inline]
	pub fn as_u32(self) -> u32 {
		self.0
	}
}

/// Handle to an interned identity string (Symbol wrapper for type safety).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegistryKey(pub Symbol);

#[derive(Debug, Default)]
pub struct InternerBuilder {
	pool: Vec<Arc<str>>,
	lookup: FxHashMap<Arc<str>, Symbol>,
}

impl InternerBuilder {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn from_frozen(f: &FrozenInterner) -> Self {
		Self {
			pool: f.pool.to_vec(),
			lookup: f.lookup.as_ref().clone(),
		}
	}

	pub fn intern(&mut self, s: &str) -> Symbol {
		if let Some(&sym) = self.lookup.get(s) {
			return sym;
		}
		debug_assert!(self.pool.len() < u32::MAX as usize);
		let sym = Symbol(self.pool.len() as u32);
		let arc: Arc<str> = Arc::from(s);
		self.pool.push(arc.clone());
		self.lookup.insert(arc, sym);
		sym
	}

	pub fn freeze(self) -> FrozenInterner {
		FrozenInterner {
			pool: Arc::from(self.pool),
			lookup: Arc::new(self.lookup),
		}
	}
}

#[derive(Debug, Clone, Default)]
pub struct FrozenInterner {
	pool: Arc<[Arc<str>]>,
	lookup: Arc<FxHashMap<Arc<str>, Symbol>>,
}

impl FrozenInterner {
	pub fn get(&self, s: &str) -> Option<Symbol> {
		self.lookup.get(s).copied()
	}

	pub fn resolve(&self, sym: Symbol) -> &str {
		if !sym.is_valid() || sym.0 as usize >= self.pool.len() {
			return "<invalid>";
		}
		&self.pool[sym.0 as usize]
	}
}

pub type Interner = FrozenInterner;

pub trait DenseId: Copy + Eq + std::hash::Hash + std::fmt::Debug + std::fmt::Display {
	const INVALID: Self;
	fn from_u32(v: u32) -> Self;
	fn as_u32(self) -> u32;
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActionId(pub u32);

impl DenseId for ActionId {
	const INVALID: Self = ActionId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		ActionId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(pub u32);

impl DenseId for CommandId {
	const INVALID: Self = CommandId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		CommandId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MotionId(pub u32);

impl DenseId for MotionId {
	const INVALID: Self = MotionId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		MotionId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextObjectId(pub u32);

impl DenseId for TextObjectId {
	const INVALID: Self = TextObjectId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		TextObjectId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OptionId(pub u32);

impl DenseId for OptionId {
	const INVALID: Self = OptionId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		OptionId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThemeId(pub u32);

impl DenseId for ThemeId {
	const INVALID: Self = ThemeId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		ThemeId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GutterId(pub u32);

impl DenseId for GutterId {
	const INVALID: Self = GutterId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		GutterId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StatuslineId(pub u32);

impl DenseId for StatuslineId {
	const INVALID: Self = StatuslineId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		StatuslineId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HookId(pub u32);

impl DenseId for HookId {
	const INVALID: Self = HookId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		HookId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayId(pub u32);

impl DenseId for OverlayId {
	const INVALID: Self = OverlayId(u32::MAX);
	fn from_u32(v: u32) -> Self {
		OverlayId(v)
	}
	fn as_u32(self) -> u32 {
		self.0
	}
}

macro_rules! impl_display_id {
    ($($t:ty),*) => {
        $(
            impl std::fmt::Display for $t {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    if self.0 == u32::MAX {
                        write!(f, "{}(INVALID)", stringify!($t))
                    } else {
                        write!(f, "{}({})", stringify!($t), self.0)
                    }
                }
            }
        )*
    };
}

impl_display_id!(
	ActionId,
	CommandId,
	MotionId,
	TextObjectId,
	OptionId,
	ThemeId,
	GutterId,
	StatuslineId,
	HookId,
	OverlayId
);

impl ActionId {
	#[inline]
	pub fn is_valid(self) -> bool {
		self.0 != u32::MAX
	}
}
