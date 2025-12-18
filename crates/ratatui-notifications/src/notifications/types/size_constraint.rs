/// Constraint on notification dimensions.
///
/// Allows specifying sizes as absolute values or percentages of available space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeConstraint {
	/// Absolute size in terminal cells/characters.
	Absolute(u16),

	/// Percentage of available screen space (0.0 to 1.0).
	Percentage(f32),
}
