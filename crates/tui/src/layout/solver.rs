//! Internal solver implementation for the layout engine.
//!
//! This module contains the constraint solver internals used by [`Layout`](super::Layout) to
//! compute layout positions. These types are not part of the public API.

use alloc::vec::Vec;

use hashbrown::HashMap;
use itertools::Itertools;
use kasuari::WeightedRelation::{EQ, GE, LE};
use kasuari::{AddConstraintError, Expression, Solver, Strength, Variable};

use super::{Constraint, Direction, Flex, Rect};

/// Multiplier that decides floating point precision when rounding.
///
/// The number of zeros in this number is the precision for the rounding of f64 to u16 in layout
/// calculations.
pub(super) const FLOAT_PRECISION_MULTIPLIER: f64 = 100.0;

/// Strength constants for the constraint solver.
///
/// These values define the priority of different constraint types when the solver resolves
/// conflicts between constraints.
pub(super) mod strengths {
	use kasuari::Strength;

	/// The strength to apply to Spacers to ensure that their sizes are equal.
	///
	/// ```text
	/// ┌     ┐┌───┐┌     ┐┌───┐┌     ┐
	///   ==x  │   │  ==x  │   │  ==x
	/// └     ┘└───┘└     ┘└───┘└     ┘
	/// ```
	pub const SPACER_SIZE_EQ: Strength = Strength::REQUIRED.div_f64(10.0);

	/// The strength to apply to Min inequality constraints.
	///
	/// ```text
	/// ┌────────┐
	/// │Min(>=x)│
	/// └────────┘
	/// ```
	pub const MIN_SIZE_GE: Strength = Strength::STRONG.mul_f64(100.0);

	/// The strength to apply to Max inequality constraints.
	///
	/// ```text
	/// ┌────────┐
	/// │Max(<=x)│
	/// └────────┘
	/// ```
	pub const MAX_SIZE_LE: Strength = Strength::STRONG.mul_f64(100.0);

	/// The strength to apply to Length constraints.
	///
	/// ```text
	/// ┌───────────┐
	/// │Length(==x)│
	/// └───────────┘
	/// ```
	pub const LENGTH_SIZE_EQ: Strength = Strength::STRONG.mul_f64(10.0);

	/// The strength to apply to Percentage constraints.
	///
	/// ```text
	/// ┌───────────────┐
	/// │Percentage(==x)│
	/// └───────────────┘
	/// ```
	pub const PERCENTAGE_SIZE_EQ: Strength = Strength::STRONG;

	/// The strength to apply to Ratio constraints.
	///
	/// ```text
	/// ┌────────────┐
	/// │Ratio(==x,y)│
	/// └────────────┘
	/// ```
	pub const RATIO_SIZE_EQ: Strength = Strength::STRONG.div_f64(10.0);

	/// The strength to apply to Max equality constraints.
	///
	/// ```text
	/// ┌────────┐
	/// │Max(==x)│
	/// └────────┘
	/// ```
	pub const MAX_SIZE_EQ: Strength = Strength::MEDIUM.mul_f64(10.0);

	/// The strength to apply to Fill growing constraints.
	///
	/// ```text
	/// ┌─────────────────────┐
	/// │<=     Fill(x)     =>│
	/// └─────────────────────┘
	/// ```
	pub const FILL_GROW: Strength = Strength::MEDIUM;

	/// The strength to apply to growing constraints.
	///
	/// ```text
	/// ┌────────────┐
	/// │<= Min(x) =>│
	/// └────────────┘
	/// ```
	pub const GROW: Strength = Strength::MEDIUM.div_f64(10.0);

	/// The strength to apply to Spacer growing constraints.
	///
	/// ```text
	/// ┌       ┐
	///  <= x =>
	/// └       ┘
	/// ```
	pub const SPACE_GROW: Strength = Strength::WEAK.mul_f64(10.0);

	/// The strength to apply to growing the size of all segments equally.
	///
	/// ```text
	/// ┌───────┐
	/// │<= x =>│
	/// └───────┘
	/// ```
	pub const ALL_SEGMENT_GROW: Strength = Strength::WEAK;
}

use strengths::{
	FILL_GROW, GROW, LENGTH_SIZE_EQ, MAX_SIZE_EQ, MAX_SIZE_LE, MIN_SIZE_GE, PERCENTAGE_SIZE_EQ,
	RATIO_SIZE_EQ, SPACE_GROW, SPACER_SIZE_EQ,
};

/// A container used by the solver inside split.
///
/// Represents a layout segment with start and end variables that the constraint solver uses to
/// compute positions.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) struct Element {
	/// Start position variable for constraint solving.
	pub start: Variable,
	/// End position variable for constraint solving.
	pub end: Variable,
}

impl From<(Variable, Variable)> for Element {
	fn from((start, end): (Variable, Variable)) -> Self {
		Self { start, end }
	}
}

impl Element {
	/// Creates a new element with fresh variables.
	#[expect(dead_code, reason = "useful for testing and debugging")]
	pub fn new() -> Self {
		Self {
			start: Variable::new(),
			end: Variable::new(),
		}
	}

	/// Returns an expression for this element's size (end - start).
	pub fn size(&self) -> Expression {
		self.end - self.start
	}

	/// Creates a constraint that the element's size is at most `size`.
	pub fn has_max_size(&self, size: u16, strength: Strength) -> kasuari::Constraint {
		self.size() | LE(strength) | (f64::from(size) * FLOAT_PRECISION_MULTIPLIER)
	}

	/// Creates a constraint that the element's size is at least `size`.
	pub fn has_min_size(&self, size: i16, strength: Strength) -> kasuari::Constraint {
		self.size() | GE(strength) | (f64::from(size) * FLOAT_PRECISION_MULTIPLIER)
	}

	/// Creates a constraint that the element's size equals `size`.
	pub fn has_int_size(&self, size: u16, strength: Strength) -> kasuari::Constraint {
		self.size() | EQ(strength) | (f64::from(size) * FLOAT_PRECISION_MULTIPLIER)
	}

	/// Creates a constraint that the element's size equals an expression.
	pub fn has_size<E: Into<Expression>>(
		&self,
		size: E,
		strength: Strength,
	) -> kasuari::Constraint {
		self.size() | EQ(strength) | size.into()
	}

	/// Creates a constraint that the element's size equals double the expression.
	pub fn has_double_size<E: Into<Expression>>(
		&self,
		size: E,
		strength: Strength,
	) -> kasuari::Constraint {
		self.size() | EQ(strength) | (size.into() * 2.0)
	}

	/// Creates a constraint that the element has zero size.
	pub fn is_empty(&self) -> kasuari::Constraint {
		self.size() | EQ(Strength::REQUIRED - Strength::WEAK) | 0.0
	}
}

/// Allow the element to represent its own size in expressions.
impl From<Element> for Expression {
	fn from(element: Element) -> Self {
		element.size()
	}
}

/// Allow the element to represent its own size in expressions.
impl From<&Element> for Expression {
	fn from(element: &Element) -> Self {
		element.size()
	}
}

/// Configure the area constraints for the solver.
pub(super) fn configure_area(
	solver: &mut Solver,
	area: Element,
	area_start: f64,
	area_end: f64,
) -> Result<(), AddConstraintError> {
	solver.add_constraint(area.start | EQ(Strength::REQUIRED) | area_start)?;
	solver.add_constraint(area.end | EQ(Strength::REQUIRED) | area_end)?;
	Ok(())
}

/// Configure constraints ensuring all variables are within the area bounds.
pub(super) fn configure_variable_in_area_constraints(
	solver: &mut Solver,
	variables: &[Variable],
	area: Element,
) -> Result<(), AddConstraintError> {
	// all variables are in the range [area.start, area.end]
	for &variable in variables {
		solver.add_constraint(variable | GE(Strength::REQUIRED) | area.start)?;
		solver.add_constraint(variable | LE(Strength::REQUIRED) | area.end)?;
	}

	Ok(())
}

/// Configure ordering constraints for variables.
///
/// ```text
/// ┌────┬───────────────────┬────┬─────variables─────┬────┬───────────────────┬────┐
/// │    │                   │    │                   │    │                   │    │
/// v    v                   v    v                   v    v                   v    v
/// ┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐┌──────────────────┐┌   ┐
///      │     Max(20)      │     │      Max(20)     │     │      Max(20)     │
/// └   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘└──────────────────┘└   ┘
/// ^    ^                   ^    ^                   ^    ^                   ^    ^
/// └v0  └v1                 └v2  └v3                 └v4  └v5                 └v6  └v7
/// ```
pub(super) fn configure_variable_constraints(
	solver: &mut Solver,
	variables: &[Variable],
) -> Result<(), AddConstraintError> {
	for (&left, &right) in variables.iter().skip(1).tuples() {
		solver.add_constraint(left | LE(Strength::REQUIRED) | right)?;
	}
	Ok(())
}

/// Configure the main layout constraints based on constraint types.
pub(super) fn configure_constraints(
	solver: &mut Solver,
	area: Element,
	segments: &[Element],
	constraints: &[Constraint],
) -> Result<(), AddConstraintError> {
	for (&constraint, &segment) in constraints.iter().zip(segments.iter()) {
		match constraint {
			Constraint::Max(max) => {
				solver.add_constraint(segment.has_max_size(max, MAX_SIZE_LE))?;
				solver.add_constraint(segment.has_int_size(max, MAX_SIZE_EQ))?;
			}
			Constraint::Min(min) => {
				solver.add_constraint(segment.has_min_size(min as i16, MIN_SIZE_GE))?;
				solver.add_constraint(segment.has_size(area, FILL_GROW))?;
			}
			Constraint::Length(length) => {
				solver.add_constraint(segment.has_int_size(length, LENGTH_SIZE_EQ))?;
			}
			Constraint::Percentage(p) => {
				let size = area.size() * f64::from(p) / 100.00;
				solver.add_constraint(segment.has_size(size, PERCENTAGE_SIZE_EQ))?;
			}
			Constraint::Ratio(num, den) => {
				// avoid division by zero by using 1 when denominator is 0
				let size = area.size() * f64::from(num) / f64::from(den.max(1));
				solver.add_constraint(segment.has_size(size, RATIO_SIZE_EQ))?;
			}
			Constraint::Fill(_) => {
				// given no other constraints, this segment will grow as much as possible.
				solver.add_constraint(segment.has_size(area, FILL_GROW))?;
			}
		}
	}
	Ok(())
}

/// Configure flex-based spacing constraints.
pub(super) fn configure_flex_constraints(
	solver: &mut Solver,
	area: Element,
	spacers: &[Element],
	flex: Flex,
	spacing: i16,
) -> Result<(), AddConstraintError> {
	let spacers_except_first_and_last = spacers.get(1..spacers.len() - 1).unwrap_or(&[]);
	let spacing_f64 = f64::from(spacing) * FLOAT_PRECISION_MULTIPLIER;
	match flex {
		Flex::SpaceAround => {
			if spacers.len() <= 2 {
				// If there are two or less spacers, fallback to Flex::SpaceEvenly
				for (left, right) in spacers.iter().tuple_combinations() {
					solver.add_constraint(left.has_size(right, SPACER_SIZE_EQ))?;
				}
				for spacer in spacers {
					solver.add_constraint(spacer.has_min_size(spacing, SPACER_SIZE_EQ))?;
					solver.add_constraint(spacer.has_size(area, SPACE_GROW))?;
				}
			} else {
				// Separate the first and last spacer from the middle ones
				let (first, rest) = spacers.split_first().unwrap();
				let (last, middle) = rest.split_last().unwrap();

				// All middle spacers should be equal in size
				for (left, right) in middle.iter().tuple_combinations() {
					solver.add_constraint(left.has_size(right, SPACER_SIZE_EQ))?;
				}

				// First and last spacers should be half the size of any middle spacer
				if let Some(first_middle) = middle.first() {
					solver.add_constraint(first_middle.has_double_size(first, SPACER_SIZE_EQ))?;
					solver.add_constraint(first_middle.has_double_size(last, SPACER_SIZE_EQ))?;
				}

				// Apply minimum size and growth constraints
				for spacer in spacers {
					solver.add_constraint(spacer.has_min_size(spacing, SPACER_SIZE_EQ))?;
					solver.add_constraint(spacer.has_size(area, SPACE_GROW))?;
				}
			}
		}

		Flex::SpaceEvenly => {
			for (left, right) in spacers.iter().tuple_combinations() {
				solver.add_constraint(left.has_size(right, SPACER_SIZE_EQ))?;
			}
			for spacer in spacers {
				solver.add_constraint(spacer.has_min_size(spacing, SPACER_SIZE_EQ))?;
				solver.add_constraint(spacer.has_size(area, SPACE_GROW))?;
			}
		}

		// All spacers excluding first and last are the same size and will grow to fill
		// any remaining space after the constraints are satisfied.
		// The first and last spacers are zero size.
		Flex::SpaceBetween => {
			for (left, right) in spacers_except_first_and_last.iter().tuple_combinations() {
				solver.add_constraint(left.has_size(right.size(), SPACER_SIZE_EQ))?;
			}
			for spacer in spacers_except_first_and_last {
				solver.add_constraint(spacer.has_min_size(spacing, SPACER_SIZE_EQ))?;
				solver.add_constraint(spacer.has_size(area, SPACE_GROW))?;
			}
			if let (Some(first), Some(last)) = (spacers.first(), spacers.last()) {
				solver.add_constraint(first.is_empty())?;
				solver.add_constraint(last.is_empty())?;
			}
		}

		Flex::Start => {
			for spacer in spacers_except_first_and_last {
				solver.add_constraint(spacer.has_size(spacing_f64, SPACER_SIZE_EQ))?;
			}
			if let (Some(first), Some(last)) = (spacers.first(), spacers.last()) {
				solver.add_constraint(first.is_empty())?;
				solver.add_constraint(last.has_size(area, GROW))?;
			}
		}
		Flex::Center => {
			for spacer in spacers_except_first_and_last {
				solver.add_constraint(spacer.has_size(spacing_f64, SPACER_SIZE_EQ))?;
			}
			if let (Some(first), Some(last)) = (spacers.first(), spacers.last()) {
				solver.add_constraint(first.has_size(area, GROW))?;
				solver.add_constraint(last.has_size(area, GROW))?;
				solver.add_constraint(first.has_size(last, SPACER_SIZE_EQ))?;
			}
		}
		Flex::End => {
			for spacer in spacers_except_first_and_last {
				solver.add_constraint(spacer.has_size(spacing_f64, SPACER_SIZE_EQ))?;
			}
			if let (Some(first), Some(last)) = (spacers.first(), spacers.last()) {
				solver.add_constraint(last.is_empty())?;
				solver.add_constraint(first.has_size(area, GROW))?;
			}
		}
	}
	Ok(())
}

/// Make every `Fill` constraint proportionally equal to each other.
///
/// This will make it fill up empty spaces equally.
///
/// ```text
/// [Fill(1), Fill(1)]
/// ┌──────┐┌──────┐
/// │abcdef││abcdef│
/// └──────┘└──────┘
///
/// [Fill(1), Fill(2)]
/// ┌──────┐┌────────────┐
/// │abcdef││abcdefabcdef│
/// └──────┘└────────────┘
/// ```
///
/// `size == base_element * scaling_factor`
pub(super) fn configure_fill_constraints(
	solver: &mut Solver,
	segments: &[Element],
	constraints: &[Constraint],
) -> Result<(), AddConstraintError> {
	for ((&left_constraint, &left_segment), (&right_constraint, &right_segment)) in constraints
		.iter()
		.zip(segments.iter())
		.filter(|(c, _)| c.is_fill() || c.is_min())
		.tuple_combinations()
	{
		let left_scaling_factor = match left_constraint {
			Constraint::Fill(scale) => f64::from(scale).max(1e-6),
			Constraint::Min(_) => 1.0,
			_ => unreachable!(),
		};
		let right_scaling_factor = match right_constraint {
			Constraint::Fill(scale) => f64::from(scale).max(1e-6),
			Constraint::Min(_) => 1.0,
			_ => unreachable!(),
		};
		solver.add_constraint(
			(right_scaling_factor * left_segment.size())
				| EQ(GROW) | (left_scaling_factor * right_segment.size()),
		)?;
	}
	Ok(())
}

/// Used instead of `f64::round` directly, to provide fallback for `no_std`.
#[cfg(feature = "std")]
#[inline]
pub(super) fn round(value: f64) -> f64 {
	value.round()
}

/// A rounding fallback for `no_std` in pure rust.
#[cfg(not(feature = "std"))]
#[inline]
pub(super) fn round(value: f64) -> f64 {
	(value + 0.5f64.copysign(value)) as i64 as f64
}

/// Type alias for the result rectangles.
pub(super) type Rects = alloc::rc::Rc<[Rect]>;

/// Convert solver variable changes to rectangles.
pub(super) fn changes_to_rects(
	changes: &HashMap<Variable, f64>,
	elements: &[Element],
	area: Rect,
	direction: Direction,
) -> Rects {
	elements
		.iter()
		.map(|element| {
			let start = changes.get(&element.start).unwrap_or(&0.0);
			let end = changes.get(&element.end).unwrap_or(&0.0);
			let start = round(round(*start) / FLOAT_PRECISION_MULTIPLIER) as u16;
			let end = round(round(*end) / FLOAT_PRECISION_MULTIPLIER) as u16;
			let size = end.saturating_sub(start);
			match direction {
				Direction::Horizontal => Rect {
					x: start,
					y: area.y,
					width: size,
					height: area.height,
				},
				Direction::Vertical => Rect {
					x: area.x,
					y: start,
					width: area.width,
					height: size,
				},
			}
		})
		.collect::<Rects>()
}

/// Debug helper for printing element positions.
///
/// Please leave this here as it's useful for debugging unit tests when we make any changes to
/// layout code - we should replace this with tracing in the future.
#[expect(dead_code, reason = "useful for debugging layout tests")]
#[cfg(feature = "std")]
pub(super) fn debug_elements(elements: &[Element], changes: &HashMap<Variable, f64>) {
	let variables = alloc::format!(
		"{:?}",
		elements
			.iter()
			.map(|e| (
				changes.get(&e.start).unwrap_or(&0.0) / FLOAT_PRECISION_MULTIPLIER,
				changes.get(&e.end).unwrap_or(&0.0) / FLOAT_PRECISION_MULTIPLIER,
			))
			.collect::<Vec<(f64, f64)>>()
	);
	std::dbg!(variables);
}
