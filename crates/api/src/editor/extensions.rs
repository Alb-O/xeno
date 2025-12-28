use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::Range;
use std::time::{Duration, Instant};

use linkme::distributed_slice;
use tome_base::color::Color;

/// Modification to apply to a style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StyleMod {
	/// Dim the foreground color by blending toward background.
	/// Value is the blend factor (0.0 = fully dimmed to bg, 1.0 = no change).
	Dim(f32),
	/// Override the foreground color.
	Fg(Color),
	/// Override the background color.
	Bg(Color),
}

impl StyleMod {
	/// Linearly interpolate between two style modifications.
	///
	/// Returns `self` blended toward `target` by `t` (0.0 = self, 1.0 = target).
	/// Only works for compatible modifications (same variant).
	pub fn lerp(self, target: Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0);
		match (self, target) {
			(StyleMod::Dim(a), StyleMod::Dim(b)) => StyleMod::Dim(a + (b - a) * t),
			(StyleMod::Fg(a), StyleMod::Fg(b)) => StyleMod::Fg(lerp_color(a, b, t)),
			(StyleMod::Bg(a), StyleMod::Bg(b)) => StyleMod::Bg(lerp_color(a, b, t)),
			// Incompatible types - snap to target
			_ => if t > 0.5 { target } else { self },
		}
	}
}

/// Lerp between two colors.
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
	match (a, b) {
		(Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
			let r = (r1 as f32 + (r2 as f32 - r1 as f32) * t) as u8;
			let g = (g1 as f32 + (g2 as f32 - g1 as f32) * t) as u8;
			let b = (b1 as f32 + (b2 as f32 - b1 as f32) * t) as u8;
			Color::Rgb(r, g, b)
		}
		// Non-RGB colors can't be lerped, snap to target
		_ => if t > 0.5 { b } else { a },
	}
}

/// A style overlay that modifies rendering for a byte range.
#[derive(Clone, Debug)]
pub struct StyleOverlay {
	/// Byte range this overlay applies to.
	pub range: Range<usize>,
	/// The style modification to apply.
	pub modification: StyleMod,
	/// Priority (higher priority overlays are applied last, overriding lower).
	pub priority: i16,
	/// Identifier for the extension that created this overlay.
	pub source: &'static str,
}

/// Easing function for animations.
#[derive(Clone, Copy, Debug, Default)]
pub enum Easing {
	/// Linear interpolation (constant speed).
	#[default]
	Linear,
	/// Ease out - starts fast, slows down at end.
	EaseOut,
	/// Ease in - starts slow, speeds up at end.
	EaseIn,
	/// Ease in-out - slow at both ends.
	EaseInOut,
}

impl Easing {
	/// Apply the easing function to a linear progress value (0.0 to 1.0).
	pub fn apply(self, t: f32) -> f32 {
		let t = t.clamp(0.0, 1.0);
		match self {
			Easing::Linear => t,
			Easing::EaseOut => 1.0 - (1.0 - t).powi(2),
			Easing::EaseIn => t.powi(2),
			Easing::EaseInOut => {
				if t < 0.5 {
					2.0 * t * t
				} else {
					1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
				}
			}
		}
	}
}

/// An animated style transition for a byte range.
#[derive(Clone, Debug)]
pub struct AnimatedOverlay {
	/// Byte range this overlay applies to.
	pub range: Range<usize>,
	/// Starting style modification.
	pub from: StyleMod,
	/// Target style modification.
	pub to: StyleMod,
	/// When the animation started.
	pub start_time: Instant,
	/// Total animation duration.
	pub duration: Duration,
	/// Easing function.
	pub easing: Easing,
	/// Priority (higher priority overlays are applied last).
	pub priority: i16,
	/// Identifier for the extension that created this overlay.
	pub source: &'static str,
}

impl AnimatedOverlay {
	/// Returns the current interpolated style modification.
	pub fn current_modification(&self) -> StyleMod {
		let elapsed = self.start_time.elapsed();
		let linear_t = (elapsed.as_secs_f32() / self.duration.as_secs_f32()).min(1.0);
		let eased_t = self.easing.apply(linear_t);
		self.from.lerp(self.to, eased_t)
	}

	/// Returns true if the animation has completed.
	pub fn is_complete(&self) -> bool {
		self.start_time.elapsed() >= self.duration
	}
}

/// Collection of style overlays for rendering.
///
/// Extensions can add overlays during their tick to modify how text is rendered.
/// Overlays are cleared each frame and must be re-added if they should persist.
#[derive(Default)]
pub struct StyleOverlays {
	overlays: Vec<StyleOverlay>,
	animated: Vec<AnimatedOverlay>,
}

impl StyleOverlays {
	pub fn new() -> Self {
		Self::default()
	}

	/// Adds an overlay for a byte range.
	pub fn add(&mut self, overlay: StyleOverlay) {
		self.overlays.push(overlay);
	}

	/// Adds an animated overlay that transitions between two styles.
	pub fn add_animated(&mut self, overlay: AnimatedOverlay) {
		self.animated.push(overlay);
	}

	/// Adds a dimming overlay for text OUTSIDE the given range.
	///
	/// This is useful for focus/zen modes where you want to dim everything
	/// except a specific region.
	pub fn dim_outside(&mut self, focus_range: Range<usize>, dim_factor: f32, source: &'static str, doc_len: usize) {
		// Add overlay for everything before the focus range
		if focus_range.start > 0 {
			self.add(StyleOverlay {
				range: 0..focus_range.start,
				modification: StyleMod::Dim(dim_factor),
				priority: 0,
				source,
			});
		}
		// Add overlay for everything after the focus range
		if focus_range.end < doc_len {
			self.add(StyleOverlay {
				range: focus_range.end..doc_len,
				modification: StyleMod::Dim(dim_factor),
				priority: 0,
				source,
			});
		}
	}

	/// Adds animated dimming overlays for text OUTSIDE the given range.
	///
	/// Transitions from `from_factor` to `to_factor` over `duration`.
	pub fn dim_outside_animated(
		&mut self,
		focus_range: Range<usize>,
		from_factor: f32,
		to_factor: f32,
		duration: Duration,
		easing: Easing,
		source: &'static str,
		doc_len: usize,
	) {
		let now = Instant::now();

		// Add animated overlay for everything before the focus range
		if focus_range.start > 0 {
			self.add_animated(AnimatedOverlay {
				range: 0..focus_range.start,
				from: StyleMod::Dim(from_factor),
				to: StyleMod::Dim(to_factor),
				start_time: now,
				duration,
				easing,
				priority: 0,
				source,
			});
		}
		// Add animated overlay for everything after the focus range
		if focus_range.end < doc_len {
			self.add_animated(AnimatedOverlay {
				range: focus_range.end..doc_len,
				from: StyleMod::Dim(from_factor),
				to: StyleMod::Dim(to_factor),
				start_time: now,
				duration,
				easing,
				priority: 0,
				source,
			});
		}
	}

	/// Clears all static overlays. Called at the start of each render frame.
	///
	/// Animated overlays are preserved until they complete.
	pub fn clear(&mut self) {
		self.overlays.clear();
		// Remove completed animations
		self.animated.retain(|a| !a.is_complete());
	}

	/// Clears all overlays including animations.
	pub fn clear_all(&mut self) {
		self.overlays.clear();
		self.animated.clear();
	}

	/// Returns the style modification for a byte position, if any.
	///
	/// Checks both static and animated overlays. If multiple overlays cover
	/// the same position, returns the one with highest priority.
	pub fn modification_at(&self, byte_pos: usize) -> Option<StyleMod> {
		// Check static overlays
		let static_mod = self
			.overlays
			.iter()
			.filter(|o| byte_pos >= o.range.start && byte_pos < o.range.end)
			.max_by_key(|o| o.priority)
			.map(|o| (o.priority, o.modification));

		// Check animated overlays
		let animated_mod = self
			.animated
			.iter()
			.filter(|o| byte_pos >= o.range.start && byte_pos < o.range.end)
			.max_by_key(|o| o.priority)
			.map(|o| (o.priority, o.current_modification()));

		// Return the highest priority one
		match (static_mod, animated_mod) {
			(Some((sp, sm)), Some((ap, am))) => {
				if ap >= sp { Some(am) } else { Some(sm) }
			}
			(Some((_, sm)), None) => Some(sm),
			(None, Some((_, am))) => Some(am),
			(None, None) => None,
		}
	}

	/// Returns true if there are any active overlays (static or animated).
	pub fn is_empty(&self) -> bool {
		self.overlays.is_empty() && self.animated.is_empty()
	}

	/// Returns true if there are any active animations.
	pub fn has_animations(&self) -> bool {
		!self.animated.is_empty()
	}
}

/// A type-safe map for storing extension state.
#[derive(Default)]
pub struct ExtensionMap {
	inner: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ExtensionMap {
	pub fn new() -> Self {
		Self::default()
	}

	/// Insert extension data. Panics in debug if already present.
	pub fn insert<T: Any + Send + Sync>(&mut self, val: T) {
		let type_id = TypeId::of::<T>();
		#[cfg(debug_assertions)]
		if self.inner.contains_key(&type_id) {
			panic!(
				"Extension state for type {} already registered",
				std::any::type_name::<T>()
			);
		}
		self.inner.insert(type_id, Box::new(val));
	}

	pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
		self.inner.get(&TypeId::of::<T>())?.downcast_ref()
	}

	pub fn get_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
		self.inner.get_mut(&TypeId::of::<T>())?.downcast_mut()
	}

	/// Get extension state or panic if not found.
	pub fn expect<T: Any + Send + Sync>(&self) -> &T {
		self.get::<T>().unwrap_or_else(|| {
			panic!(
				"Extension state for type {} missing",
				std::any::type_name::<T>()
			)
		})
	}

	/// Get extension state mutably or panic if not found.
	pub fn expect_mut<T: Any + Send + Sync>(&mut self) -> &mut T {
		self.get_mut::<T>().unwrap_or_else(|| {
			panic!(
				"Extension state for type {} missing",
				std::any::type_name::<T>()
			)
		})
	}

	/// Get extension state, initializing it if missing.
	pub fn get_or_init<T: Any + Send + Sync, F: FnOnce() -> T>(&mut self, f: F) -> &mut T {
		let type_id = TypeId::of::<T>();
		self.inner.entry(type_id).or_insert_with(|| Box::new(f()));
		self.inner
			.get_mut(&type_id)
			.unwrap()
			.downcast_mut()
			.unwrap()
	}
}

/// Definition for extension initialization.
pub struct ExtensionInitDef {
	/// Extension identifier (for debugging).
	pub id: &'static str,
	/// Priority (lower runs first).
	pub priority: i16,
	/// Initialization function.
	pub init: fn(&mut ExtensionMap),
}

/// Registry of all terminal-side extensions.
#[distributed_slice]
pub static EXTENSIONS: [ExtensionInitDef];

pub struct ExtensionTickDef {
	/// Priority (lower runs first).
	pub priority: i16,
	pub tick: fn(&mut crate::editor::Editor),
}

/// Extensions that need to run on every editor tick.
#[distributed_slice]
pub static TICK_EXTENSIONS: [ExtensionTickDef];

/// Definition for render-time extension updates.
///
/// These run at the start of each render frame, after events have been
/// processed. Use this for style overlays and other render-time state
/// that needs to reflect the current cursor position.
pub struct ExtensionRenderDef {
	/// Priority (lower runs first).
	pub priority: i16,
	/// Update function called before each render.
	pub update: fn(&mut crate::editor::Editor),
}

/// Extensions that need to update state before each render.
///
/// Unlike TICK_EXTENSIONS (which run at the start of the event loop),
/// these run right before rendering, ensuring they see the latest
/// cursor position after mouse clicks and other events.
#[distributed_slice]
pub static RENDER_EXTENSIONS: [ExtensionRenderDef];
