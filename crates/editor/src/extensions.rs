use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::Range;
use std::time::Duration;

use xeno_tui::animation::Animatable;
use xeno_tui::style::Color;

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

impl Animatable for StyleMod {
	fn lerp(&self, target: &Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0);
		match (self, target) {
			(StyleMod::Dim(a), StyleMod::Dim(b)) => StyleMod::Dim(a.lerp(b, t)),
			(StyleMod::Fg(a), StyleMod::Fg(b)) => StyleMod::Fg(a.lerp(b, t)),
			(StyleMod::Bg(a), StyleMod::Bg(b)) => StyleMod::Bg(a.lerp(b, t)),
			// Incompatible types - snap to target
			_ => {
				if t > 0.5 {
					*target
				} else {
					*self
				}
			}
		}
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

// Re-export Easing from xeno_tui::animation for convenience
pub use xeno_tui::animation::Easing;

/// An animated style transition for a byte range.
#[derive(Clone, Debug)]
pub struct AnimatedOverlay {
	/// Byte range this overlay applies to.
	pub range: Range<usize>,
	/// The underlying tween for the style modification.
	tween: xeno_tui::animation::Tween<StyleMod>,
	/// Priority (higher priority overlays are applied last).
	pub priority: i16,
	/// Identifier for the extension that created this overlay.
	pub source: &'static str,
}

impl AnimatedOverlay {
	/// Creates a new animated overlay.
	pub fn new(
		range: Range<usize>,
		from: StyleMod,
		to: StyleMod,
		duration: Duration,
		easing: Easing,
		priority: i16,
		source: &'static str,
	) -> Self {
		Self {
			range,
			tween: xeno_tui::animation::Tween::new(from, to, duration).with_easing(easing),
			priority,
			source,
		}
	}

	/// Returns the starting style modification.
	pub fn from(&self) -> StyleMod {
		self.tween.start
	}

	/// Returns the target style modification.
	pub fn to(&self) -> StyleMod {
		self.tween.end
	}

	/// Returns the animation duration.
	pub fn duration(&self) -> Duration {
		self.tween.duration
	}

	/// Returns the easing function.
	pub fn easing(&self) -> Easing {
		self.tween.easing
	}

	/// Returns the current interpolated style modification.
	pub fn current_modification(&self) -> StyleMod {
		self.tween.value()
	}

	/// Returns true if the animation has completed.
	pub fn is_complete(&self) -> bool {
		self.tween.is_complete()
	}
}

/// Collection of style overlays for rendering.
///
/// Overlays can be added during rendering to modify how text is displayed.
/// Static overlays are cleared each frame; animated overlays persist until complete.
#[derive(Default, Clone)]
pub struct StyleOverlays {
	/// Static overlays that apply for a single frame.
	overlays: Vec<StyleOverlay>,
	/// Animated overlays that transition over time and persist until complete.
	animated: Vec<AnimatedOverlay>,
}

impl StyleOverlays {
	/// Creates a new empty overlay collection.
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
	pub fn dim_outside(
		&mut self,
		focus_range: Range<usize>,
		dim_factor: f32,
		source: &'static str,
		doc_len: usize,
	) {
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
		// Add animated overlay for everything before the focus range
		if focus_range.start > 0 {
			self.add_animated(AnimatedOverlay::new(
				0..focus_range.start,
				StyleMod::Dim(from_factor),
				StyleMod::Dim(to_factor),
				duration,
				easing,
				0,
				source,
			));
		}
		// Add animated overlay for everything after the focus range
		if focus_range.end < doc_len {
			self.add_animated(AnimatedOverlay::new(
				focus_range.end..doc_len,
				StyleMod::Dim(from_factor),
				StyleMod::Dim(to_factor),
				duration,
				easing,
				0,
				source,
			));
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
				if ap >= sp {
					Some(am)
				} else {
					Some(sm)
				}
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
	/// Map from type ID to boxed extension state.
	inner: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ExtensionMap {
	/// Creates a new empty extension map.
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

	/// Returns a reference to extension state of type T, if present.
	pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
		self.inner.get(&TypeId::of::<T>())?.downcast_ref()
	}

	/// Returns a mutable reference to extension state of type T, if present.
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
