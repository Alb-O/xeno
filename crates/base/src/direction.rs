//! Directional types for navigation and layout operations.

/// Spatial direction for focus navigation between views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialDirection {
	Left,
	Right,
	Up,
	Down,
}

/// Axis for split operations.
///
/// Names refer to the **divider line orientation**, matching Vim/Helix:
/// - `Horizontal` = horizontal divider → windows stacked top/bottom
/// - `Vertical` = vertical divider → windows side-by-side left/right
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
	Horizontal,
	Vertical,
}

/// Sequential direction for ordered traversal (buffers, search matches).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SeqDirection {
	Next,
	Prev,
}
