#[inline(always)]
pub(crate) fn match_unordered_with_typos<T, I, F>(needle: I, max_typos: u16, mut contains: F) -> bool
where
	I: IntoIterator<Item = T>,
	F: FnMut(T) -> bool,
{
	let mut typos = 0usize;
	let max_typos = max_typos as usize;

	for needle_char in needle {
		if !contains(needle_char) {
			typos += 1;
			if typos > max_typos {
				return false;
			}
		}
	}

	true
}
