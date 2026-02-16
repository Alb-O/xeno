/// Safely converts a `usize` index to `u32` for registry storage.
///
/// # Panics
///
/// Panics if `idx` exceeds `u32::MAX`.
pub(crate) fn u32_index(idx: usize, what: &'static str) -> u32 {
	u32::try_from(idx).unwrap_or_else(|_| panic!("{} index overflow: {}", what, idx))
}
