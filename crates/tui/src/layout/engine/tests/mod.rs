use alloc::borrow::ToOwned;
use alloc::vec;
use alloc::vec::Vec;

use super::*;

#[test]
// The compiler will optimize out the comparisons, but this ensures that the constants are
// defined in the correct order of priority.
pub fn strength_is_valid() {
	use crate::layout::solver::strengths::*;
	assert!(SPACER_SIZE_EQ > MAX_SIZE_LE);
	assert!(MAX_SIZE_LE > MAX_SIZE_EQ);
	assert!(MIN_SIZE_GE == MAX_SIZE_LE);
	assert!(MAX_SIZE_LE > LENGTH_SIZE_EQ);
	assert!(LENGTH_SIZE_EQ > PERCENTAGE_SIZE_EQ);
	assert!(PERCENTAGE_SIZE_EQ > RATIO_SIZE_EQ);
	assert!(RATIO_SIZE_EQ > MAX_SIZE_EQ);
	assert!(MIN_SIZE_GE > FILL_GROW);
	assert!(FILL_GROW > GROW);
	assert!(GROW > SPACE_GROW);
	assert!(SPACE_GROW > ALL_SEGMENT_GROW);
}

#[test]
#[cfg(feature = "layout-cache")]
fn cache_size() {
	LAYOUT_CACHE.with_borrow(|cache| {
		assert_eq!(cache.cap().get(), Layout::DEFAULT_CACHE_SIZE);
	});

	Layout::init_cache(NonZeroUsize::new(10).unwrap());
	LAYOUT_CACHE.with_borrow(|cache| {
		assert_eq!(cache.cap().get(), 10);
	});
}

#[test]
fn default() {
	assert_eq!(
		Layout::default(),
		Layout {
			direction: Direction::Vertical,
			margin: Margin::new(0, 0),
			constraints: vec![],
			flex: Flex::default(),
			spacing: Spacing::default(),
		}
	);
}

#[test]
fn new() {
	// array
	let fixed_size_array = [Constraint::Min(0)];
	let layout = Layout::new(Direction::Horizontal, fixed_size_array);
	assert_eq!(layout.direction, Direction::Horizontal);
	assert_eq!(layout.constraints, [Constraint::Min(0)]);

	// array_ref
	#[expect(clippy::needless_borrows_for_generic_args)] // backwards compatibility test
	let layout = Layout::new(Direction::Horizontal, &[Constraint::Min(0)]);
	assert_eq!(layout.direction, Direction::Horizontal);
	assert_eq!(layout.constraints, [Constraint::Min(0)]);

	// vec
	let layout = Layout::new(Direction::Horizontal, vec![Constraint::Min(0)]);
	assert_eq!(layout.direction, Direction::Horizontal);
	assert_eq!(layout.constraints, [Constraint::Min(0)]);

	// vec_ref
	#[expect(clippy::needless_borrows_for_generic_args)] // backwards compatibility test
	let layout = Layout::new(Direction::Horizontal, &(vec![Constraint::Min(0)]));
	assert_eq!(layout.direction, Direction::Horizontal);
	assert_eq!(layout.constraints, [Constraint::Min(0)]);

	// iterator
	let layout = Layout::new(Direction::Horizontal, iter::once(Constraint::Min(0)));
	assert_eq!(layout.direction, Direction::Horizontal);
	assert_eq!(layout.constraints, [Constraint::Min(0)]);
}

#[test]
fn vertical() {
	assert_eq!(
		Layout::vertical([Constraint::Min(0)]),
		Layout {
			direction: Direction::Vertical,
			margin: Margin::new(0, 0),
			constraints: vec![Constraint::Min(0)],
			flex: Flex::default(),
			spacing: Spacing::default(),
		}
	);
}

#[test]
fn horizontal() {
	assert_eq!(
		Layout::horizontal([Constraint::Min(0)]),
		Layout {
			direction: Direction::Horizontal,
			margin: Margin::new(0, 0),
			constraints: vec![Constraint::Min(0)],
			flex: Flex::default(),
			spacing: Spacing::default(),
		}
	);
}

/// The purpose of this test is to ensure that layout can be constructed with any type that
/// implements `IntoIterator<Item = AsRef<Constraint>>`.
#[test]
fn constraints() {
	const CONSTRAINTS: [Constraint; 2] = [Constraint::Min(0), Constraint::Max(10)];
	let fixed_size_array = CONSTRAINTS;
	assert_eq!(
		Layout::default().constraints(fixed_size_array).constraints,
		CONSTRAINTS,
		"constraints should be settable with an array"
	);

	let slice_of_fixed_size_array = &CONSTRAINTS;
	assert_eq!(
		Layout::default()
			.constraints(slice_of_fixed_size_array)
			.constraints,
		CONSTRAINTS,
		"constraints should be settable with a slice"
	);

	let vec = CONSTRAINTS.to_vec();
	let slice_of_vec = vec.as_slice();
	assert_eq!(
		Layout::default().constraints(slice_of_vec).constraints,
		CONSTRAINTS,
		"constraints should be settable with a slice"
	);

	assert_eq!(
		Layout::default().constraints(vec).constraints,
		CONSTRAINTS,
		"constraints should be settable with a Vec"
	);

	let iter = CONSTRAINTS.iter();
	assert_eq!(
		Layout::default().constraints(iter).constraints,
		CONSTRAINTS,
		"constraints should be settable with an iter"
	);

	let iterator = CONSTRAINTS.iter().map(ToOwned::to_owned);
	assert_eq!(
		Layout::default().constraints(iterator).constraints,
		CONSTRAINTS,
		"constraints should be settable with an iterator"
	);

	let iterator_ref = CONSTRAINTS.iter().map(AsRef::as_ref);
	assert_eq!(
		Layout::default().constraints(iterator_ref).constraints,
		CONSTRAINTS,
		"constraints should be settable with an iterator of refs"
	);
}

#[test]
fn direction() {
	assert_eq!(
		Layout::default().direction(Direction::Horizontal).direction,
		Direction::Horizontal
	);
	assert_eq!(
		Layout::default().direction(Direction::Vertical).direction,
		Direction::Vertical
	);
}

#[test]
fn margins() {
	assert_eq!(Layout::default().margin(10).margin, Margin::new(10, 10));
	assert_eq!(
		Layout::default().horizontal_margin(10).margin,
		Margin::new(10, 0)
	);
	assert_eq!(
		Layout::default().vertical_margin(10).margin,
		Margin::new(0, 10)
	);
	assert_eq!(
		Layout::default()
			.horizontal_margin(10)
			.vertical_margin(20)
			.margin,
		Margin::new(10, 20)
	);
}

#[test]
fn flex() {
	assert_eq!(Layout::default().flex, Flex::Start);
	assert_eq!(Layout::default().flex(Flex::Center).flex, Flex::Center);
}

#[test]
fn spacing() {
	assert_eq!(Layout::default().spacing(10).spacing, Spacing::Space(10));
	assert_eq!(Layout::default().spacing(0).spacing, Spacing::Space(0));
	assert_eq!(Layout::default().spacing(-10).spacing, Spacing::Overlap(10));
}

mod split;
