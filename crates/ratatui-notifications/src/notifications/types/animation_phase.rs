/// Animation phase tracking.
///
/// Represents the current stage of a notification's lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationPhase {
	#[default]
	Pending,
	SlidingIn,
	Expanding,
	FadingIn,
	Dwelling,
	SlidingOut,
	Collapsing,
	FadingOut,
	Finished,
}
