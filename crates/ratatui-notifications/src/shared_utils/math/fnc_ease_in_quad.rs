/// Applies quadratic ease-in easing to a linear progress value.
///
/// The ease-in function starts slowly and accelerates toward the end.
///
/// # Arguments
///
/// * `t` - The linear progress value (typically 0.0 to 1.0)
///
/// # Returns
///
/// The eased progress value
///
/// # Examples
///
/// ```ignore
/// // Internal function
/// let result = ease_in_quad(0.5);
/// assert_eq!(result, 0.25);
/// ```
#[inline]
pub fn ease_in_quad(t: f32) -> f32 {
	t * t
}
