/// Animation style for notification entry and exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Animation {
	/// Slide animation from a direction (default).
	///
	/// Notification slides in from the specified direction and slides out
	/// when dismissed. Smooth and commonly used.
	#[default]
	Slide,

	/// Expand/collapse animation.
	///
	/// Notification expands from anchor point when entering and collapses
	/// when exiting. Creates a growing/shrinking effect.
	ExpandCollapse,

	/// Fade animation.
	///
	/// Notification fades in when appearing and fades out when dismissed.
	/// Subtle and non-intrusive.
	Fade,
}
