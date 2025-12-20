#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionKind {
	Id,
	Name,
	Alias,
	Trigger,
}

impl std::fmt::Display for CollisionKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Id => write!(f, "ID"),
			Self::Name => write!(f, "name"),
			Self::Alias => write!(f, "alias"),
			Self::Trigger => write!(f, "trigger"),
		}
	}
}

pub struct Collision<T: 'static> {
	pub kind: CollisionKind,
	pub key: String,
	pub winner: &'static T,
	pub shadowed: &'static T,
}
