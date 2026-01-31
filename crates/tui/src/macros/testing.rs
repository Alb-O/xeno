//! Test helper macros to reduce boilerplate in widget and layout tests.
//!
//! These macros are only available in test builds and provide convenient ways to:
//! - Test widget rendering against expected buffer output
//! - Test enum Display/FromStr roundtrips
//! - Define data-driven layout constraint tests

/// Asserts that rendering a widget to a buffer produces the expected output.
///
/// # Examples
///
/// Basic usage with a widget and expected lines:
/// ```ignore
/// render_test!(Block::bordered(), (10, 3), [
///     "┌────────┐",
///     "│        │",
///     "└────────┘",
/// ]);
/// ```
///
/// With a custom rect position:
/// ```ignore
/// render_test!(Block::bordered(), Rect::new(0, 0, 10, 3), [
///     "┌────────┐",
///     "│        │",
///     "└────────┘",
/// ]);
/// ```
#[macro_export]
macro_rules! render_test {
	// Form: widget, (width, height), [lines...]
	($widget:expr, ($width:expr, $height:expr), [$($line:expr),* $(,)?]) => {{
		let area = $crate::layout::Rect::new(0, 0, $width, $height);
		let mut buf = $crate::buffer::Buffer::empty(area);
		$crate::widgets::Widget::render($widget, area, &mut buf);
		let expected = $crate::buffer::Buffer::with_lines([$($line),*]);
		assert_eq!(buf, expected);
	}};

	// Form: widget, Rect, [lines...]
	($widget:expr, $rect:expr, [$($line:expr),* $(,)?]) => {{
		let area = $rect;
		let mut buf = $crate::buffer::Buffer::empty(area);
		$crate::widgets::Widget::render($widget, area, &mut buf);
		let expected = $crate::buffer::Buffer::with_lines([$($line),*]);
		assert_eq!(buf, expected);
	}};
}

/// Asserts that rendering a stateful widget produces the expected output.
///
/// # Examples
///
/// ```ignore
/// let mut state = TableState::new().with_selected(Some(0));
/// render_stateful_test!(table, &mut state, (15, 3), [
///     ">>Cell1 Cell2  ",
///     "  Cell3 Cell4  ",
///     "               ",
/// ]);
/// ```
#[macro_export]
macro_rules! render_stateful_test {
	($widget:expr, $state:expr, ($width:expr, $height:expr), [$($line:expr),* $(,)?]) => {{
		let area = $crate::layout::Rect::new(0, 0, $width, $height);
		let mut buf = $crate::buffer::Buffer::empty(area);
		$crate::widgets::StatefulWidget::render($widget, area, &mut buf, $state);
		let expected = $crate::buffer::Buffer::with_lines([$($line),*]);
		assert_eq!(buf, expected);
	}};

	($widget:expr, $state:expr, $rect:expr, [$($line:expr),* $(,)?]) => {{
		let area = $rect;
		let mut buf = $crate::buffer::Buffer::empty(area);
		$crate::widgets::StatefulWidget::render($widget, area, &mut buf, $state);
		let expected = $crate::buffer::Buffer::with_lines([$($line),*]);
		assert_eq!(buf, expected);
	}};
}

/// Generates Display and FromStr roundtrip tests for an enum.
///
/// This macro generates tests that verify:
/// 1. Each variant's Display output matches its name
/// 2. Parsing the name produces the correct variant
/// 3. Parsing an empty string returns an error
///
/// # Examples
///
/// ```ignore
/// enum_display_from_str_tests!(BorderType, [
///     Plain,
///     Rounded,
///     Double,
///     Thick,
/// ]);
/// ```
///
/// This expands to individual test assertions for each variant.
#[macro_export]
macro_rules! enum_display_from_str_tests {
	($enum_ty:ty, [$($variant:ident),* $(,)?]) => {
		// Test Display
		$(
			assert_eq!(format!("{}", <$enum_ty>::$variant), stringify!($variant));
		)*

		// Test FromStr
		$(
			assert_eq!(stringify!($variant).parse::<$enum_ty>(), Ok(<$enum_ty>::$variant));
		)*

		// Test empty string error
		assert!("".parse::<$enum_ty>().is_err());
	};
}

/// Generates a block of enum Display tests.
///
/// # Examples
///
/// ```ignore
/// enum_display_tests!(BorderType, [
///     Plain,
///     Rounded,
///     Double,
/// ]);
/// ```
#[macro_export]
macro_rules! enum_display_tests {
	($enum_ty:ty, [$($variant:ident),* $(,)?]) => {
		$(
			assert_eq!(format!("{}", <$enum_ty>::$variant), stringify!($variant));
		)*
	};
}

/// Generates a block of enum FromStr tests.
///
/// # Examples
///
/// ```ignore
/// enum_from_str_tests!(BorderType, [
///     Plain,
///     Rounded,
///     Double,
/// ]);
/// ```
#[macro_export]
macro_rules! enum_from_str_tests {
	($enum_ty:ty, [$($variant:ident),* $(,)?]) => {
		$(
			assert_eq!(stringify!($variant).parse::<$enum_ty>(), Ok(<$enum_ty>::$variant));
		)*
	};
}

/// Defines layout constraint test cases in a compact table format.
///
/// This macro generates test functions that verify layout behavior for
/// different constraints.
///
/// # Examples
///
/// ```ignore
/// layout_constraint_tests! {
///     test_name: percentage_tests,
///     width: 10,
///     cases: [
///         ([Percentage(0), Percentage(0)], "bbbbbbbbbb"),
///         ([Percentage(0), Percentage(25)], "bbbbbbbbbb"),
///         ([Percentage(10), Percentage(0)], "abbbbbbbbb"),
///     ]
/// }
/// ```
#[macro_export]
macro_rules! layout_constraint_tests {
	{
		test_name: $name:ident,
		width: $width:expr,
		cases: [
			$(
				([$($constraint:expr),* $(,)?], $expected:expr)
			),* $(,)?
		]
	} => {
		#[test]
		fn $name() {
			use $crate::layout::{Constraint, Layout, Rect};

			let width: u16 = $width;

			$(
				{
					let constraints: &[Constraint] = &[$($constraint),*];
					let expected: &str = $expected;
					letters(constraints, width, expected);
				}
			)*
		}
	};
}

/// Defines multiple layout test cases with shared setup.
///
/// A more compact way to define many constraint test cases that share
/// the same test harness function.
///
/// # Examples
///
/// ```ignore
/// layout_cases!(letters, [
///     (10, [Percentage(0), Percentage(0)], "bbbbbbbbbb"),
///     (10, [Percentage(0), Percentage(25)], "bbbbbbbbbb"),
///     (10, [Percentage(0), Percentage(0)], "          "),
/// ]);
/// ```
#[macro_export]
macro_rules! layout_cases {
	($harness:ident, [
		$(
			($width:expr, [$($constraint:expr),* $(,)?], $expected:expr)
		),* $(,)?
	]) => {
		$(
			$harness(&[$($constraint),*], $width, $expected);
		)*
	};
}

/// Generates a test function that checks layout splits produce expected ranges.
///
/// # Examples
///
/// ```ignore
/// layout_range_tests!(constraint_length, 100, [
///     ([Length(25), Min(100)], [0..0, 0..100]),
///     ([Length(25), Min(0)], [0..25, 25..100]),
/// ]);
/// ```
#[macro_export]
macro_rules! layout_range_tests {
	($name:ident, $width:expr, [
		$(
			([$($constraint:expr),* $(,)?], [$($range:expr),* $(,)?])
		),* $(,)?
	]) => {
		#[test]
		fn $name() {
			use $crate::layout::{Constraint, Layout, Rect};

			let rect = Rect::new(0, 0, $width, 1);

			$(
				{
					let constraints = vec![$($constraint),*];
					let expected: Vec<core::ops::Range<u16>> = vec![$($range),*];
					let ranges: Vec<_> = Layout::horizontal(&constraints)
						.split(rect)
						.iter()
						.map(|r| r.left()..r.right())
						.collect();
					assert_eq!(ranges, expected);
				}
			)*
		}
	};
}

/// Generates tests for layout with position and width tuples.
///
/// # Examples
///
/// ```ignore
/// layout_pos_width_tests!(basic_spacing, 100, [
///     ([(0, 20), (20, 20), (40, 20)], [Length(20), Length(20), Length(20)]),
///     ([(0, 30), (30, 70)], [Length(30), Min(1)]),
/// ]);
/// ```
#[macro_export]
macro_rules! layout_pos_width_tests {
	($name:ident, $rect_width:expr, [
		$(
			([$(($x:expr, $w:expr)),* $(,)?], [$($constraint:expr),* $(,)?])
		),* $(,)?
	]) => {
		#[test]
		fn $name() {
			use $crate::layout::{Constraint, Layout, Rect};

			let rect = Rect::new(0, 0, $rect_width, 1);

			$(
				{
					let expected: Vec<(u16, u16)> = vec![$(($x, $w)),*];
					let constraints = vec![$($constraint),*];
					let r = Layout::horizontal(&constraints)
						.split(rect);
					let result: Vec<(u16, u16)> = r
						.iter()
						.map(|r| (r.x, r.width))
						.collect();
					assert_eq!(result, expected);
				}
			)*
		}
	};
}

/// Compact test case definition for rstest-style parameterized tests.
///
/// Generates individual test cases that can be used with a test harness.
///
/// # Examples
///
/// ```ignore
/// test_cases!(render_truncates_emoji, render_truncates, [
///     (HorizontalAlignment::Left, 4, "1234"),
///     (HorizontalAlignment::Left, 5, "1234 "),
///     (HorizontalAlignment::Right, 4, "7890"),
/// ]);
/// ```
#[macro_export]
macro_rules! test_cases {
	($name:ident, $harness:ident, [
		$(($($arg:expr),* $(,)?)),* $(,)?
	]) => {
		#[test]
		fn $name() {
			$(
				$harness($($arg),*);
			)*
		}
	};
}

/// Inline test cases without generating a new test function.
/// Use within an existing test function to run multiple cases through a harness.
///
/// # Examples
///
/// ```ignore
/// #[test]
/// fn my_test() {
///     fn check(a: i32, b: i32, expected: i32) {
///         assert_eq!(a + b, expected);
///     }
///     
///     run_cases!(check, [
///         (1, 2, 3),
///         (4, 5, 9),
///     ]);
/// }
/// ```
#[macro_export]
macro_rules! run_cases {
	($harness:ident, [
		$(($($arg:expr),* $(,)?)),* $(,)?
	]) => {
		$(
			$harness($($arg),*);
		)*
	};
}

#[cfg(test)]
mod tests {
	use crate::widgets::Block;

	#[test]
	fn test_render_test_macro() {
		render_test!(
			Block::bordered().border_type(crate::widgets::BorderType::Plain),
			(10, 3),
			["┌────────┐", "│        │", "└────────┘",]
		);
	}

	#[test]
	fn test_enum_display_from_str() {
		use crate::widgets::BorderType;

		enum_display_from_str_tests!(BorderType, [Plain, Rounded, Double, Thick, Padded, Stripe,]);
	}

	#[test]
	fn test_run_cases() {
		fn check_add(a: i32, b: i32, expected: i32) {
			assert_eq!(a + b, expected);
		}

		run_cases!(check_add, [(1, 2, 3), (10, 20, 30), (-5, 5, 0),]);
	}
}
