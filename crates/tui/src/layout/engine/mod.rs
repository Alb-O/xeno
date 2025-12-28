use alloc::vec::Vec;
use core::array::TryFromSliceError;
use core::iter;
#[cfg(feature = "layout-cache")]
use core::num::NonZeroUsize;

use hashbrown::HashMap;
use itertools::Itertools;
use kasuari::{AddConstraintError, Solver, Variable};
#[cfg(feature = "layout-cache")]
use lru::LruCache;

use super::solver::strengths::ALL_SEGMENT_GROW;
use super::solver::{
	Element, FLOAT_PRECISION_MULTIPLIER, Rects, changes_to_rects, configure_area,
	configure_constraints, configure_fill_constraints, configure_flex_constraints,
	configure_variable_constraints, configure_variable_in_area_constraints,
};
pub use super::spacing::Spacing;
use crate::layout::{Constraint, Direction, Flex, Margin, Rect};

type Segments = super::solver::Rects;
type Spacers = super::solver::Rects;
// The solution to a Layout solve contains two `Rects`, where `Rects` is effectively a `[Rect]`.
//
// 1. `[Rect]` that contains positions for the segments corresponding to user provided constraints
// 2. `[Rect]` that contains spacers around the user provided constraints
//
// <------------------------------------80 px------------------------------------->
// ┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐
//   1  │        a         │  2  │         b        │  3  │         c        │  4
// └   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘
//
// Number of spacers will always be one more than number of segments.
#[cfg(feature = "layout-cache")]
type Cache = LruCache<(Rect, Layout), (Segments, Spacers)>;

#[cfg(feature = "layout-cache")]
std::thread_local! {
	static LAYOUT_CACHE: core::cell::RefCell<Cache> = core::cell::RefCell::new(Cache::new(
		NonZeroUsize::new(Layout::DEFAULT_CACHE_SIZE).unwrap(),
	));
}

/// Layout engine for dividing terminal space using constraints and direction.
///
/// Splits rectangular areas using constraints (length, ratio, percentage, fill, min, max),
/// direction (horizontal/vertical), margin, flex, and spacing. Uses the [`kasuari`] linear
/// constraint solver. Results are cached in a thread-local LRU cache.
///
/// # Example
///
/// ```rust
/// use crate::layout::{Constraint, Layout, Rect};
///
/// let layout = Layout::vertical([Constraint::Length(5), Constraint::Fill(1)]);
/// let [top, bottom] = layout.areas(Rect::new(0, 0, 80, 24));
/// ```
///
/// [`kasuari`]: https://crates.io/crates/kasuari
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Layout {
	direction: Direction,
	constraints: Vec<Constraint>,
	margin: Margin,
	flex: Flex,
	spacing: Spacing,
}

impl Layout {
	/// This is a somewhat arbitrary size for the layout cache based on adding the columns and rows
	/// on my laptop's terminal (171+51 = 222) and doubling it for good measure and then adding a
	/// bit more to make it a round number. This gives enough entries to store a layout for every
	/// row and every column, twice over, which should be enough for most apps. For those that need
	/// more, the cache size can be set with `Layout::init_cache()` (requires the `layout-cache`
	/// feature).
	#[cfg(feature = "layout-cache")]
	pub const DEFAULT_CACHE_SIZE: usize = 500;

	/// Creates a new layout with the given direction and constraints.
	/// Constraints can be arrays, slices, vectors, or iterators. `u16` also works via `Into<Constraint>`.
	pub fn new<I>(direction: Direction, constraints: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Constraint>,
	{
		Self {
			direction,
			constraints: constraints.into_iter().map(Into::into).collect(),
			..Self::default()
		}
	}

	/// Creates a new vertical layout.
	pub fn vertical<I>(constraints: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Constraint>,
	{
		Self::new(Direction::Vertical, constraints.into_iter().map(Into::into))
	}

	/// Creates a new horizontal layout.
	pub fn horizontal<I>(constraints: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Constraint>,
	{
		Self::new(
			Direction::Horizontal,
			constraints.into_iter().map(Into::into),
		)
	}

	/// Initialize the cache with a custom size (default: [`Self::DEFAULT_CACHE_SIZE`]).
	#[cfg(feature = "layout-cache")]
	pub fn init_cache(cache_size: NonZeroUsize) {
		LAYOUT_CACHE.with_borrow_mut(|cache| cache.resize(cache_size));
	}

	/// Set the direction of the layout.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn direction(mut self, direction: Direction) -> Self {
		self.direction = direction;
		self
	}

	/// Sets the constraints. Note: mixing percentages/ratios with other constraints may
	/// yield indeterminate results due to constraint solver behavior.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn constraints<I>(mut self, constraints: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Constraint>,
	{
		self.constraints = constraints.into_iter().map(Into::into).collect();
		self
	}

	/// Set uniform margin on all sides.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn margin(mut self, margin: u16) -> Self {
		self.margin = Margin {
			horizontal: margin,
			vertical: margin,
		};
		self
	}

	/// Set horizontal margin (left and right).
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn horizontal_margin(mut self, horizontal: u16) -> Self {
		self.margin.horizontal = horizontal;
		self
	}

	/// Set vertical margin (top and bottom).
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn vertical_margin(mut self, vertical: u16) -> Self {
		self.margin.vertical = vertical;
		self
	}

	/// Set flex behavior for space distribution. See [`Flex`] for variants.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub const fn flex(mut self, flex: Flex) -> Self {
		self.flex = flex;
		self
	}

	/// Set spacing between segments (positive for gaps, negative for overlaps).
	/// Not applied for single segments or `SpaceAround`/`SpaceEvenly`/`SpaceBetween` flex modes.
	#[must_use = "method moves the value of self and returns the modified value"]
	pub fn spacing<T>(mut self, spacing: T) -> Self
	where
		T: Into<Spacing>,
	{
		self.spacing = spacing.into();
		self
	}

	/// Split into N sub-rects. Panics if constraint count != N. Use [`Self::split`] for runtime count.
	pub fn areas<const N: usize>(&self, area: Rect) -> [Rect; N] {
		let areas = self.split(area);
		areas.as_ref().try_into().unwrap_or_else(|_| {
			panic!(
				"invalid number of rects: expected {N}, found {}",
				areas.len()
			)
		})
	}

	/// Like [`Self::areas`] but returns `Result` instead of panicking.
	pub fn try_areas<const N: usize>(&self, area: Rect) -> Result<[Rect; N], TryFromSliceError> {
		self.split(area).as_ref().try_into()
	}

	/// Get spacer rectangles between layout areas. Panics if count != N.
	pub fn spacers<const N: usize>(&self, area: Rect) -> [Rect; N] {
		let (_, spacers) = self.split_with_spacers(area);
		spacers
			.as_ref()
			.try_into()
			.expect("invalid number of rects")
	}

	/// Split area into sub-rects. Results are cached. Use [`Self::areas`] for compile-time count.
	pub fn split(&self, area: Rect) -> Rects {
		self.split_with_spacers(area).0
	}

	/// Like [`Self::split`] but also returns spacer rectangles between areas.
	pub fn split_with_spacers(&self, area: Rect) -> (Segments, Spacers) {
		let split = || self.try_split(area).expect("failed to split");

		#[cfg(feature = "layout-cache")]
		{
			LAYOUT_CACHE.with_borrow_mut(|cache| {
				let key = (area, self.clone());
				cache.get_or_insert(key, split).clone()
			})
		}

		#[cfg(not(feature = "layout-cache"))]
		split()
	}

	fn try_split(&self, area: Rect) -> Result<(Segments, Spacers), AddConstraintError> {
		// To take advantage of all of [`kasuari`] features, we would want to store the `Solver` in
		// one of the fields of the Layout struct. And we would want to set it up such that we could
		// add or remove constraints as and when needed.
		// The advantage of doing it as described above is that it would allow users to
		// incrementally add and remove constraints efficiently.
		// Solves will just one constraint different would not need to resolve the entire layout.
		//
		// The disadvantage of this approach is that it requires tracking which constraints were
		// added, and which variables they correspond to.
		// This will also require introducing and maintaining the API for users to do so.
		//
		// Currently we don't support that use case and do not intend to support it in the future,
		// and instead we require that the user re-solve the layout every time they call `split`.
		// To minimize the time it takes to solve the same problem over and over again, we
		// cache the `Layout` struct along with the results.
		//
		// `try_split` is the inner method in `split` that is called only when the LRU cache doesn't
		// match the key. So inside `try_split`, we create a new instance of the solver.
		//
		// This is equivalent to storing the solver in `Layout` and calling `solver.reset()` here.
		let mut solver = Solver::new();

		let inner_area = area.inner(self.margin);
		let (area_start, area_end) = match self.direction {
			Direction::Horizontal => (
				f64::from(inner_area.x) * FLOAT_PRECISION_MULTIPLIER,
				f64::from(inner_area.right()) * FLOAT_PRECISION_MULTIPLIER,
			),
			Direction::Vertical => (
				f64::from(inner_area.y) * FLOAT_PRECISION_MULTIPLIER,
				f64::from(inner_area.bottom()) * FLOAT_PRECISION_MULTIPLIER,
			),
		};

		// ```plain
		// <───────────────────────────────────area_size──────────────────────────────────>
		// ┌─area_start                                                          area_end─┐
		// V                                                                              V
		// ┌────┬───────────────────┬────┬─────variables─────┬────┬───────────────────┬────┐
		// │    │                   │    │                   │    │                   │    │
		// V    V                   V    V                   V    V                   V    V
		// ┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐
		//      │     Max(20)      │     │      Max(20)     │     │      Max(20)     │
		// └   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘
		// ^    ^                   ^    ^                   ^    ^                   ^    ^
		// │    │                   │    │                   │    │                   │    │
		// └─┬──┶━━━━━━━━━┳━━━━━━━━━┵─┬──┶━━━━━━━━━┳━━━━━━━━━┵─┬──┶━━━━━━━━━┳━━━━━━━━━┵─┬──┘
		//   │            ┃           │            ┃           │            ┃           │
		//   └────────────╂───────────┴────────────╂───────────┴────────────╂──Spacers──┘
		//                ┃                        ┃                        ┃
		//                ┗━━━━━━━━━━━━━━━━━━━━━━━━┻━━━━━━━━Segments━━━━━━━━┛
		// ```

		let variable_count = self.constraints.len() * 2 + 2;
		let variables = iter::repeat_with(Variable::new)
			.take(variable_count)
			.collect_vec();
		let spacers = variables
			.iter()
			.tuples()
			.map(|(a, b)| Element::from((*a, *b)))
			.collect_vec();
		let segments = variables
			.iter()
			.skip(1)
			.tuples()
			.map(|(a, b)| Element::from((*a, *b)))
			.collect_vec();

		let flex = self.flex;

		let spacing = match self.spacing {
			Spacing::Space(x) => x as i16,
			Spacing::Overlap(x) => -(x as i16),
		};

		let constraints = &self.constraints;

		let area_size = Element::from((*variables.first().unwrap(), *variables.last().unwrap()));
		configure_area(&mut solver, area_size, area_start, area_end)?;
		configure_variable_in_area_constraints(&mut solver, &variables, area_size)?;
		configure_variable_constraints(&mut solver, &variables)?;
		configure_flex_constraints(&mut solver, area_size, &spacers, flex, spacing)?;
		configure_constraints(&mut solver, area_size, &segments, constraints, flex)?;
		configure_fill_constraints(&mut solver, &segments, constraints, flex)?;

		if !flex.is_legacy() {
			for (left, right) in segments.iter().tuple_windows() {
				solver.add_constraint(left.has_size(right, ALL_SEGMENT_GROW))?;
			}
		}

		// `solver.fetch_changes()` can only be called once per solve
		let changes: HashMap<Variable, f64> = solver.fetch_changes().iter().copied().collect();
		// debug_elements(&segments, &changes);
		// debug_elements(&spacers, &changes);

		let segment_rects = changes_to_rects(&changes, &segments, inner_area, self.direction);
		let spacer_rects = changes_to_rects(&changes, &spacers, inner_area, self.direction);

		Ok((segment_rects, spacer_rects))
	}
}

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
