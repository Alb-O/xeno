//! Integration tests for separator hover animations.
//!
//! These tests verify that the separator hover effect includes smooth fade-in
//! and fade-out animations, and that velocity-based suppression works correctly.

mod helpers;

use std::time::Duration;

use helpers::{insert_text, reset_test_file, tome_cmd_with_file_named, workspace_dir};
use kitty_test_harness::{
	AnsiColor, extract_row_colors_parsed, find_separator_rows_at_col, find_vertical_separator_col,
	kitty_send_keys, pause_briefly, require_kitty, run_with_timeout, send_mouse_move,
	wait_for_screen_text_clean, with_kitty_capture,
};
use termwiz::input::{KeyCode, Modifiers};

const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Creates a vertical split (Ctrl+w s) - left/right panes with vertical separator.
fn create_vertical_split(kitty: &kitty_test_harness::KittyHarness) {
	kitty_send_keys!(kitty, (KeyCode::Char('w'), Modifiers::CTRL));
	kitty_send_keys!(kitty, KeyCode::Char('s'));
	pause_briefly();
}

/// Check if an RGB color is strictly between two other colors (indicating a lerped value).
fn is_lerped_color(color: (u8, u8, u8), start: (u8, u8, u8), end: (u8, u8, u8)) -> bool {
	let in_range = |v: u8, a: u8, b: u8| -> bool {
		let (min, max) = if a <= b { (a, b) } else { (b, a) };
		v > min && v < max
	};

	let r_between = in_range(color.0, start.0, end.0);
	let g_between = in_range(color.1, start.1, end.1);
	let b_between = in_range(color.2, start.2, end.2);

	let different_from_start = color != start;
	let different_from_end = color != end;

	different_from_start && different_from_end && (r_between || g_between || b_between)
}

/// Extract the separator's foreground color from a row's colors.
///
/// The separator color typically appears once (unique), while gutter colors
/// appear on both left and right sides.
fn find_separator_fg_color(colors: &[AnsiColor]) -> Option<(u8, u8, u8)> {
	use std::collections::HashMap;

	let mut color_counts: HashMap<(u8, u8, u8), usize> = HashMap::new();
	for c in colors.iter().filter(|c| c.is_foreground) {
		if let Some(rgb) = c.rgb {
			*color_counts.entry(rgb).or_insert(0) += 1;
		}
	}

	// Prefer unique colors (separator appears once, gutter appears twice)
	if let Some((&rgb, _)) = color_counts.iter().find(|&(_, &count)| count == 1) {
		return Some(rgb);
	}

	// Fallback to last foreground color
	colors
		.iter()
		.filter(|c| c.is_foreground && c.rgb.is_some())
		.filter_map(|c| c.rgb)
		.last()
}

/// Tests that hovering over a separator triggers an animated fade-in.
///
/// Verifies that during the fade-in animation, we observe intermediate (lerped)
/// color values between the normal and hover colors.
#[serial_test::serial]
#[test]
fn separator_hover_shows_lerped_animation() {
	if !require_kitty() {
		return;
	}

	let file = "kitty-test-separator-anim.txt";
	reset_test_file(file);
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &tome_cmd_with_file_named(file), |kitty| {
			pause_briefly();

			insert_text(kitty, "LEFT PANE");
			pause_briefly();
			create_vertical_split(kitty);
			insert_text(kitty, "RIGHT PANE");
			pause_briefly();

			let (raw_before, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
					clean.contains("LEFT PANE") && clean.contains("RIGHT PANE")
				});

			let sep_col = find_vertical_separator_col(&clean);
			assert!(sep_col.is_some(), "Should have a vertical separator");

			let sep_col = sep_col.unwrap() as u16;
			let sep_rows = find_separator_rows_at_col(&clean, sep_col as usize);
			let sep_row = sep_rows.get(sep_rows.len() / 2).copied().unwrap_or(10) as u16;

			let colors_before = extract_row_colors_parsed(&raw_before, sep_row as usize);
			let normal_color = find_separator_fg_color(&colors_before);

			// Ensure mouse is away from separator
			send_mouse_move(kitty, 10, sep_row);
			std::thread::sleep(Duration::from_millis(100));

			// Move to separator - animation starts (120ms duration)
			send_mouse_move(kitty, sep_col, sep_row);

			// Sample during animation to catch lerped color
			let mut during_color = None;
			for _ in 0..5 {
				std::thread::sleep(Duration::from_millis(30));
				let (raw_sample, _) =
					wait_for_screen_text_clean(kitty, Duration::from_millis(50), |_, _| true);
				let colors_sample = extract_row_colors_parsed(&raw_sample, sep_row as usize);
				let sample_color = find_separator_fg_color(&colors_sample);

				if during_color.is_none() && sample_color.is_some() {
					during_color = sample_color;
				}
				if let (Some(normal), Some(sample)) = (normal_color, sample_color) {
					if sample != normal {
						during_color = sample_color;
						break;
					}
				}
			}

			// Wait for animation to complete
			std::thread::sleep(Duration::from_millis(100));

			let (raw_after, _) =
				wait_for_screen_text_clean(kitty, Duration::from_millis(100), |_, _| true);
			let colors_after = extract_row_colors_parsed(&raw_after, sep_row as usize);
			let hover_color = find_separator_fg_color(&colors_after);

			let normal_color = normal_color.expect("Should have normal separator color");
			let during_color = during_color.expect("Should have color during animation");
			let hover_color = hover_color.expect("Should have final hover color");

			assert!(
				is_lerped_color(during_color, normal_color, hover_color),
				"Color during fade-in should be lerped.\n\
				 Normal: {:?}, During: {:?}, Hover: {:?}",
				normal_color,
				during_color,
				hover_color
			);

			assert_ne!(normal_color, hover_color, "Normal and hover should differ");

			// Cleanup
			send_mouse_move(kitty, 10, sep_row);
			pause_briefly();

			let (_, clean_after) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(2), |_, clean| {
					clean.contains("LEFT PANE") && clean.contains("RIGHT PANE")
				});
			assert!(
				find_vertical_separator_col(&clean_after).is_some(),
				"Separator should still exist"
			);
		});
	});
}

/// Tests that moving mouse away from separator triggers a fade-OUT animation.
///
/// Verifies that during the fade-out animation, we observe intermediate (lerped)
/// color values between the hover and normal colors.
#[serial_test::serial]
#[test]
fn separator_fadeout_shows_lerped_animation() {
	if !require_kitty() {
		return;
	}

	let file = "kitty-test-separator-fadeout.txt";
	reset_test_file(file);
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &tome_cmd_with_file_named(file), |kitty| {
			pause_briefly();

			insert_text(kitty, "LEFT");
			pause_briefly();
			create_vertical_split(kitty);
			insert_text(kitty, "RIGHT");
			pause_briefly();

			let (raw_normal, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_, clean| {
					clean.contains("LEFT") && clean.contains("RIGHT")
				});

			let sep_col = find_vertical_separator_col(&clean);
			assert!(sep_col.is_some(), "Should have separator");

			let sep_col = sep_col.unwrap() as u16;
			let sep_rows = find_separator_rows_at_col(&clean, sep_col as usize);
			let sep_row = sep_rows.get(sep_rows.len() / 2).copied().unwrap_or(10) as u16;

			let colors_normal = extract_row_colors_parsed(&raw_normal, sep_row as usize);
			let normal_color = find_separator_fg_color(&colors_normal);

			// Hover and wait for fade-in to complete
			send_mouse_move(kitty, sep_col, sep_row);
			std::thread::sleep(Duration::from_millis(200));

			let (raw_hovered, _) =
				wait_for_screen_text_clean(kitty, Duration::from_millis(100), |_, _| true);
			let colors_hovered = extract_row_colors_parsed(&raw_hovered, sep_row as usize);
			let hover_color = find_separator_fg_color(&colors_hovered);

			// Move away - fade-out animation starts
			send_mouse_move(kitty, 10, sep_row);

			// Sample during fade-out to catch lerped color
			let mut fadeout_color = None;
			for _ in 0..5 {
				std::thread::sleep(Duration::from_millis(30));
				let (raw_sample, _) =
					wait_for_screen_text_clean(kitty, Duration::from_millis(50), |_, _| true);
				let colors_sample = extract_row_colors_parsed(&raw_sample, sep_row as usize);
				let sample_color = find_separator_fg_color(&colors_sample);

				if fadeout_color.is_none() && sample_color.is_some() {
					fadeout_color = sample_color;
				}
				if let (Some(normal), Some(hover), Some(sample)) =
					(normal_color, hover_color, sample_color)
				{
					if sample != normal && sample != hover {
						fadeout_color = sample_color;
						break;
					}
				}
			}

			let normal_color = normal_color.expect("Should have normal separator color");
			let hover_color = hover_color.expect("Should have hovered separator color");
			let fadeout_color = fadeout_color.expect("Should have color during fadeout");

			assert!(
				is_lerped_color(fadeout_color, hover_color, normal_color),
				"Color during fade-out should be lerped.\n\
				 Hover: {:?}, Fadeout: {:?}, Normal: {:?}",
				hover_color,
				fadeout_color,
				normal_color
			);
		});
	});
}

/// Tests that fast mouse movement over a separator does NOT trigger hover.
///
/// Velocity-based suppression should prevent hover effects when the mouse
/// moves quickly across the separator.
#[serial_test::serial]
#[test]
fn fast_mouse_suppresses_separator_hover() {
	if !require_kitty() {
		return;
	}

	let file = "kitty-test-separator-fast.txt";
	reset_test_file(file);
	run_with_timeout(TEST_TIMEOUT, || {
		with_kitty_capture(&workspace_dir(), &tome_cmd_with_file_named(file), |kitty| {
			pause_briefly();

			insert_text(kitty, "LEFT");
			pause_briefly();
			create_vertical_split(kitty);
			insert_text(kitty, "RIGHT");
			pause_briefly();

			let (raw_baseline, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_, clean| {
					clean.contains("LEFT") && clean.contains("RIGHT")
				});

			let sep_col = find_vertical_separator_col(&clean);
			assert!(sep_col.is_some(), "Should have separator");

			let sep_col = sep_col.unwrap() as u16;
			let sep_rows = find_separator_rows_at_col(&clean, sep_col as usize);
			let sep_row = sep_rows.get(sep_rows.len() / 2).copied().unwrap_or(10) as u16;

			let colors_baseline = extract_row_colors_parsed(&raw_baseline, sep_row as usize);
			let baseline_color = find_separator_fg_color(&colors_baseline);

			// Move mouse quickly across the separator
			for x in 0..80u16 {
				send_mouse_move(kitty, x, sep_row);
				std::thread::sleep(Duration::from_millis(5));
			}

			// Capture immediately - hover should NOT be active
			let (raw_after_fast, _) =
				wait_for_screen_text_clean(kitty, Duration::from_millis(100), |_, _| true);
			let colors_after_fast = extract_row_colors_parsed(&raw_after_fast, sep_row as usize);
			let after_fast_color = find_separator_fg_color(&colors_after_fast);

			// Colors should remain at baseline (no hover triggered)
			if let (Some(baseline), Some(after)) = (baseline_color, after_fast_color) {
				assert_eq!(
					baseline, after,
					"Fast mouse movement should not trigger hover"
				);
			}

			let (_, clean_after) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(2), |_, clean| {
					clean.contains("LEFT") && clean.contains("RIGHT")
				});
			assert!(
				find_vertical_separator_col(&clean_after).is_some(),
				"Separator should still be rendered"
			);
		});
	});
}
