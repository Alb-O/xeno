//! Structured test event logging for integration tests.
//!
//! This module provides a structured logging system for debugging integration tests
//! where the editor runs inside a kitty terminal and stderr is not visible to the
//! test runner.
//!
//! # Usage
//!
//! Set `XENO_TEST_LOG` environment variable to a file path to enable logging:
//!
//! ```bash
//! XENO_TEST_LOG=/tmp/test.jsonl cargo test
//! ```
//!
//! Then use the logging macros in code:
//!
//! ```ignore
//! use xeno_api::test_event;
//!
//! test_event!(
//!     "separator_animation",
//!     event = "fade_in",
//!     intensity = 0.5,
//!     fg = (100, 100, 100),
//!     bg = (0, 0, 0)
//! );
//! ```
//!
//! Events are written as newline-delimited JSON for easy parsing.

use std::io::Write;
use std::sync::OnceLock;

use serde::Serialize;

/// Returns the test log file path if `XENO_TEST_LOG` is set.
fn test_log_path() -> Option<&'static str> {
	static PATH: OnceLock<Option<String>> = OnceLock::new();
	PATH.get_or_init(|| std::env::var("XENO_TEST_LOG").ok())
		.as_deref()
}

/// Writes a structured event to the test log file.
///
/// This is the low-level function used by the `test_event!` macro.
/// Prefer using the macro for cleaner syntax.
pub fn write_test_event<T: Serialize>(event: &T) {
	let Some(path) = test_log_path() else {
		return;
	};

	let Ok(json) = serde_json::to_string(event) else {
		return;
	};

	let Ok(mut file) = std::fs::OpenOptions::new()
		.create(true)
		.append(true)
		.open(path)
	else {
		return;
	};

	let _ = writeln!(file, "{}", json);
}

/// RGB color tuple for structured logging.
pub type Rgb = (u8, u8, u8);

/// Separator animation events.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SeparatorAnimationEvent {
	/// Animation started (fade-in or fade-out).
	#[serde(rename = "separator_animation_start")]
	Start {
		/// Animation direction.
		direction: AnimationDirection,
	},
	/// Animation frame rendered.
	#[serde(rename = "separator_animation_frame")]
	Frame {
		/// Current animation intensity (0.0 to 1.0).
		intensity: f32,
		/// Foreground color RGB.
		fg: Rgb,
		/// Background color RGB.
		bg: Rgb,
	},
}

/// Animation direction.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationDirection {
	/// Fading in (hovering).
	FadeIn,
	/// Fading out (leaving).
	FadeOut,
}

impl SeparatorAnimationEvent {
	/// Log animation start event.
	pub fn start(direction: AnimationDirection) {
		write_test_event(&Self::Start { direction });
	}

	/// Log animation frame event.
	pub fn frame(intensity: f32, fg: Rgb, bg: Rgb) {
		write_test_event(&Self::Frame { intensity, fg, bg });
	}
}
