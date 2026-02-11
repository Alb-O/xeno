use std::collections::HashMap;

use super::*;

#[test]
fn test_rect_policy_top_center_clamping() {
	let screen = Rect::new(0, 0, 100, 50);
	let roles = HashMap::new();
	let policy = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 80,
		min_width: 20,
		y_frac: (1, 4),
		height: 10,
	};

	let rect = policy.resolve_opt(screen, &roles).unwrap();
	assert_eq!(rect.width, 50);
	assert_eq!(rect.x, 25);
	assert_eq!(rect.y, 12);
	assert_eq!(rect.height, 10);
}

#[test]
fn test_rect_policy_overflow_protection() {
	let screen = Rect::new(0, 0, 100, 50);
	let roles = HashMap::new();

	// Case: width_percent > 100
	let policy_overflow_pct = RectPolicy::TopCenter {
		width_percent: 200,
		max_width: 500,
		min_width: 20,
		y_frac: (1, 4),
		height: 10,
	};
	// Should be clamped to screen width (100)
	let rect = policy_overflow_pct.resolve_opt(screen, &roles).unwrap();
	assert_eq!(rect.width, 100);
	assert_eq!(rect.x, 0);

	// Case: huge y_frac to simulate overflow/wrapping if u16 was used
	let policy_huge_y = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 80,
		min_width: 20,
		y_frac: (1000, 1), // 50 * 1000 = 50000, which fits in u16 but is way off screen
		height: 10,
	};
	// Should now clamp to screen_bottom - height (50 - 10 = 40)
	let rect = policy_huge_y.resolve_opt(screen, &roles).unwrap();
	assert_eq!(rect.y, 40);
	assert_eq!(rect.height, 10);
}

#[test]
fn test_rect_policy_div_by_zero() {
	let screen = Rect::new(0, 0, 100, 50);
	let roles = HashMap::new();
	let policy = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 80,
		min_width: 20,
		y_frac: (1, 0), // Division by zero
		height: 10,
	};
	assert!(policy.resolve_opt(screen, &roles).is_none());
}

#[test]
fn test_rect_policy_min_gt_max() {
	let screen = Rect::new(0, 0, 100, 50);
	let roles = HashMap::new();
	let policy = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 20, // max < min
		min_width: 80,
		y_frac: (1, 4),
		height: 10,
	};
	// Should swap min/max effectively, so min=20, max=80
	// width_percent 50 of 100 is 50. 50 is between 20 and 80.
	let rect = policy.resolve_opt(screen, &roles).unwrap();
	assert_eq!(rect.width, 50);
}

#[test]
fn test_rect_policy_below_clamping() {
	let screen = Rect::new(0, 0, 100, 50);
	let mut roles = HashMap::new();

	// Anchor almost off screen at bottom
	roles.insert(WindowRole::Input, Rect::new(10, 45, 80, 5));

	let policy = RectPolicy::Below(WindowRole::Input, 2, 10);
	// y = 45 + 5 + 2 = 52. Screen h = 50. 52 >= 50. Should be None.
	assert!(policy.resolve_opt(screen, &roles).is_none());

	// Test horizontal clamping for Below
	// Anchor is wider than screen?
	roles.insert(WindowRole::Custom("Wide"), Rect::new(0, 10, 200, 10)); // 200 width
	let policy_wide = RectPolicy::Below(WindowRole::Custom("Wide"), 5, 10);

	// Should clamp width to screen width (100)
	let rect = policy_wide.resolve_opt(screen, &roles).unwrap();
	assert_eq!(rect.width, 100);
	assert_eq!(rect.x, 0);
}

#[test]
fn test_screen_offset_handling() {
	let screen = Rect::new(10, 10, 100, 50); // Screen starts at 10,10
	let roles = HashMap::new();
	let policy = RectPolicy::TopCenter {
		width_percent: 50, // 50 chars
		max_width: 80,
		min_width: 20,
		y_frac: (0, 1), // Top
		height: 10,
	};

	let rect = policy.resolve_opt(screen, &roles).unwrap();
	// Width 50. Centered in 100 is offset 25.
	// X should be screen.x (10) + 25 = 35.
	assert_eq!(rect.x, 35);
	// Y should be screen.y (10) + 0 = 10.
	assert_eq!(rect.y, 10);
}

#[test]
fn test_top_center_respects_non_zero_container_origin() {
	let container = Rect::new(4, 30, 72, 10);
	let roles = HashMap::new();
	let policy = RectPolicy::TopCenter {
		width_percent: 100,
		max_width: u16::MAX,
		min_width: 1,
		y_frac: (0, 1),
		height: 1,
	};

	let rect = policy
		.resolve_opt(container, &roles)
		.expect("policy should resolve inside non-zero-origin container");
	assert_eq!(rect.x, 4);
	assert_eq!(rect.y, 30);
	assert_eq!(rect.width, 72);
	assert_eq!(rect.height, 1);
}
