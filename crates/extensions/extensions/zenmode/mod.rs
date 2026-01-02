//! Zen Mode extension for Evildoer.
//!
//! Provides a focus mode that dims syntax highlighting outside the current
//! tree-sitter element (function, struct, impl, etc.) containing the cursor.
//! This helps maintain focus on the code section being actively edited.
//!
//! ## Usage
//!
//! Toggle zen mode with the `:zen` command (or `:zenmode`, `:focus`).
//! When enabled, text outside the current function/struct/impl will be dimmed,
//! helping you focus on the code you're actively editing.
//!
//! ## How it works
//!
//! On each editor tick, the extension:
//! 1. Finds the tree-sitter node at the cursor position
//! 2. Walks up the syntax tree to find a "significant" container (function, struct, etc.)
//! 3. Registers a style overlay that dims everything outside that container

use std::ops::Range;

use evildoer_api::editor::Editor;
use evildoer_macro::extension;
use evildoer_registry::commands::{CommandContext, CommandError, CommandOutcome};

/// Primary focus node types - these are the main code units we want to focus on.
///
/// These represent logical code units like functions, types, and impl blocks.
/// When the cursor is inside one of these, we dim everything outside it.
const PRIMARY_FOCUS_NODES: &[&str] = &[
	// Functions (highest priority)
	"function_item",
	"function_definition",
	"function_declaration",
	"method_definition",
	"method_declaration",
	"closure_expression",
	"lambda_expression",
	"arrow_function",
	"function",
	"struct_item",
	"struct_definition",
	"enum_item",
	"enum_definition",
	"union_item",
	"trait_item",
	"impl_item",
	"class_definition",
	"class_declaration",
	"class_body",
	"interface_declaration",
	"mod_item",
	"module",
	"namespace_definition",
];

/// Secondary focus nodes - larger containers that we fall back to.
///
/// If we can't find a primary focus node, we look for these.
/// These are typically top-level declarations or statement blocks.
const SECONDARY_FOCUS_NODES: &[&str] = &[
	"const_item",
	"static_item",
	"type_alias",
	"use_declaration",
	"extern_crate",
	"macro_definition",
	"macro_rules",
	// Statement-level blocks (less preferred)
	"match_expression",
	"if_expression",
	"if_statement",
	"for_expression",
	"for_statement",
	"while_expression",
	"while_statement",
	"loop_expression",
	"block",
	"statement_block",
];

/// Checks if a node kind is a primary focus container.
fn is_primary_focus(kind: &str) -> bool {
	PRIMARY_FOCUS_NODES.contains(&kind)
}

/// Checks if a node kind is a secondary focus container.
fn is_secondary_focus(kind: &str) -> bool {
	SECONDARY_FOCUS_NODES.contains(&kind)
}

use std::time::{Duration, Instant};

use evildoer_api::editor::extensions::Easing;

/// Default animation duration for focus transitions between nodes.
const TRANSITION_DURATION: Duration = Duration::from_millis(150);

/// Animation duration when undimming the entire document (leaving all nodes).
const UNDIM_ALL_DURATION: Duration = Duration::from_millis(400);

/// Default debounce duration for transitions between focus nodes.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(50);

/// Longer debounce for undimming the entire document.
/// This prevents flickering when rapidly moving through code.
const UNDIM_ALL_DEBOUNCE: Duration = Duration::from_millis(300);

/// State for the zen mode extension.
pub struct ZenmodeState {
	/// Whether zen mode is currently enabled.
	pub enabled: bool,
	/// The byte range of the current focus node (if any).
	/// When zen mode is active, text outside this range is dimmed.
	pub focus_range: Option<Range<usize>>,
	/// Previous focus range (for animation).
	prev_focus_range: Option<Range<usize>>,
	/// Pending focus range (waiting for debounce).
	pending_focus_range: Option<Range<usize>>,
	/// Whether the pending change is an "undim all" (None target).
	pending_is_undim_all: bool,
	/// When a new focus range was first requested (for debounce).
	pending_since: Option<Instant>,
	/// When the focus range last changed (for animation).
	last_change: Option<Instant>,
	/// Whether the current animation is an "undim all" transition.
	is_undim_all_animation: bool,
	/// Dimming factor for out-of-focus text (0.0 = invisible, 1.0 = normal).
	pub dim_factor: f32,
	/// Animation duration for focus transitions between nodes.
	pub transition_duration: Duration,
	/// Animation duration when undimming the entire document.
	pub undim_all_duration: Duration,
	/// Debounce duration for transitions between focus nodes.
	pub debounce_duration: Duration,
	/// Debounce duration for undimming the entire document.
	pub undim_all_debounce: Duration,
	/// Whether animations are enabled.
	pub animate: bool,
}

impl Default for ZenmodeState {
	fn default() -> Self {
		Self::new()
	}
}

#[extension(id = "zenmode", priority = 50)]
impl ZenmodeState {
	#[init]
	pub fn new() -> Self {
		Self {
			enabled: false,
			focus_range: None,
			prev_focus_range: None,
			pending_focus_range: None,
			pending_is_undim_all: false,
			pending_since: None,
			last_change: None,
			is_undim_all_animation: false,
			dim_factor: 0.35,
			transition_duration: TRANSITION_DURATION,
			undim_all_duration: UNDIM_ALL_DURATION,
			debounce_duration: DEBOUNCE_DURATION,
			undim_all_debounce: UNDIM_ALL_DEBOUNCE,
			animate: true,
		}
	}

	/// Requests a focus range change with debouncing.
	///
	/// The change is not applied immediately - it's held as pending until
	/// the debounce duration passes without further changes. This prevents
	/// flickering when the cursor moves rapidly across different nodes.
	///
	/// Transitions to `None` (undim all) use a longer debounce to prevent
	/// jarring flashes when quickly moving through code.
	pub fn request_focus_range(&mut self, new_range: Option<Range<usize>>) {
		// If this is the same as current, clear any pending change
		if self.focus_range == new_range {
			self.pending_focus_range = None;
			self.pending_since = None;
			self.pending_is_undim_all = false;
			return;
		}

		// If new range is contained within current range, ignore it.
		// This prevents flickering when moving within a function to a nested
		// block that would technically be a "different" focus node.
		if let (Some(current), Some(new)) = (&self.focus_range, &new_range)
			&& new.start >= current.start
			&& new.end <= current.end
		{
			// New range is inside current - keep current, clear pending
			self.pending_focus_range = None;
			self.pending_since = None;
			self.pending_is_undim_all = false;
			return;
		}

		let is_undim_all = new_range.is_none() && self.focus_range.is_some();

		// If this is a new pending range, start the debounce timer
		let dominated_range = self.pending_focus_range.as_ref().cloned();
		if dominated_range != new_range {
			self.pending_focus_range = new_range;
			self.pending_since = Some(Instant::now());
			self.pending_is_undim_all = is_undim_all;
		}
	}

	/// Commits any pending focus range change if the debounce period has passed.
	///
	/// Returns true if a change was committed (animation should start).
	pub fn commit_pending(&mut self) -> bool {
		let debounce_time = if self.pending_is_undim_all {
			self.undim_all_debounce
		} else {
			self.debounce_duration
		};

		let debounce_elapsed = match self.pending_since {
			Some(t) => t.elapsed() >= debounce_time,
			None => false,
		};

		if debounce_elapsed && self.pending_focus_range != self.focus_range {
			self.prev_focus_range = self.focus_range.take();
			self.focus_range = self.pending_focus_range.take();
			self.last_change = Some(Instant::now());
			self.is_undim_all_animation = self.pending_is_undim_all;
			self.pending_since = None;
			self.pending_is_undim_all = false;
			return true;
		}

		false
	}

	/// Returns the effective focus range for rendering.
	///
	/// During debounce, returns the current (not pending) range.
	pub fn effective_range(&self) -> Option<&Range<usize>> {
		self.focus_range.as_ref()
	}

	/// Returns true if there's a pending change waiting for debounce.
	pub fn has_pending(&self) -> bool {
		self.pending_since.is_some() && self.pending_focus_range != self.focus_range
	}

	/// Returns the animation duration for the current transition.
	fn current_animation_duration(&self) -> Duration {
		if self.is_undim_all_animation {
			self.undim_all_duration
		} else {
			self.transition_duration
		}
	}

	/// Returns the animation progress (0.0 to 1.0).
	/// Returns 1.0 if no animation is in progress or animations are disabled.
	pub fn animation_progress(&self) -> f32 {
		if !self.animate {
			return 1.0;
		}
		match self.last_change {
			Some(t) => {
				let elapsed = t.elapsed();
				let duration = self.current_animation_duration();
				(elapsed.as_secs_f32() / duration.as_secs_f32()).min(1.0)
			}
			None => 1.0,
		}
	}

	/// Returns true if an animation is currently in progress.
	pub fn is_animating(&self) -> bool {
		self.animate && self.animation_progress() < 1.0
	}

	/// Returns true if the current animation is an "undim all" transition.
	pub fn is_undim_all(&self) -> bool {
		self.is_undim_all_animation
	}

	/// Returns the previous focus range (before the last change).
	pub fn prev_range(&self) -> Option<&Range<usize>> {
		self.prev_focus_range.as_ref()
	}

	/// Toggles zen mode on/off.
	pub fn toggle(&mut self) {
		self.enabled = !self.enabled;
		if !self.enabled {
			self.focus_range = None;
		}
	}

	/// Returns true if the given byte position is within the focus range.
	pub fn is_in_focus(&self, byte_pos: usize) -> bool {
		match &self.focus_range {
			Some(range) => byte_pos >= range.start && byte_pos < range.end,
			None => true,
		}
	}

	#[command(
		"zenmode",
		aliases = ["zen", "focus"],
		description = "Toggle zen/focus mode for syntax highlighting"
	)]
	fn toggle_command(
		&mut self,
		ctx: &mut CommandContext<'_>,
	) -> Result<CommandOutcome, CommandError> {
		self.toggle();
		let status = if self.enabled { "enabled" } else { "disabled" };
		ctx.info(&format!("Zen mode {}", status));
		Ok(CommandOutcome::Ok)
	}

	#[render(priority = 100)]
	fn update_zenmode(&mut self, editor: &mut Editor) {
		update_zenmode_state(editor, self);
	}
}

/// Finds the best focus node by walking up the tree from the cursor position.
///
/// Uses a two-pass approach:
/// 1. First, look for primary focus nodes (functions, types, impl blocks)
/// 2. If none found, look for secondary focus nodes (statements, blocks)
///
/// This ensures we always find a reasonable container even when the cursor
/// is inside nested expressions like string literals.
fn find_focus_range(
	syntax: &evildoer_language::syntax::Syntax,
	cursor_byte: u32,
) -> Option<Range<usize>> {
	// Find the smallest named node containing the cursor
	let start_node = syntax.named_descendant_for_byte_range(cursor_byte, cursor_byte)?;

	// Track the best secondary match as we walk up (in case we don't find a primary)
	let mut secondary_match: Option<Range<usize>> = None;

	// Single pass: walk up looking for primary nodes, but remember secondary matches
	let mut current = start_node;
	loop {
		let kind = current.kind();

		// Primary focus nodes take precedence - return immediately
		if is_primary_focus(kind) {
			return Some(current.start_byte() as usize..current.end_byte() as usize);
		}

		// Remember secondary matches as fallback
		if secondary_match.is_none() && is_secondary_focus(kind) {
			secondary_match = Some(current.start_byte() as usize..current.end_byte() as usize);
		}

		match current.parent() {
			Some(parent) => current = parent,
			None => break,
		}
	}

	// No primary found - use secondary if we found one
	secondary_match
}

/// Updates the focus range based on cursor position and syntax tree,
/// and registers style overlays to dim out-of-focus regions.
fn update_zenmode_state(editor: &mut Editor, state: &mut ZenmodeState) {
	// First, read the state to check if enabled and get config
	let enabled = state.enabled;
	let dim_factor = state.dim_factor;
	let animate = state.animate;
	let current_range = state.focus_range.clone();

	if !enabled {
		return;
	}

	// Get cursor position and compute focus range while holding doc lock
	let new_focus_range = {
		let buffer = editor.buffer();
		let doc = buffer.doc();
		let syntax = match &doc.syntax {
			Some(s) => s,
			None => {
				drop(doc);
				// Clear focus range when no syntax tree
				state.request_focus_range(None);
				state.commit_pending();
				return;
			}
		};

		// Convert cursor position to byte position
		let cursor_byte = doc.content.char_to_byte(buffer.cursor) as u32;
		let cursor_byte_usize = cursor_byte as usize;

		// Stability check: if cursor is still within the current focus range,
		// don't look for a new node - just keep the current one.
		// This prevents flickering when tree-sitter returns slightly different
		// nodes on different frames.
		if let Some(ref range) = current_range {
			if cursor_byte_usize >= range.start && cursor_byte_usize < range.end {
				// Cursor still in current range - keep it
				Some(range.clone())
			} else {
				// Cursor left the range - find a new one
				find_focus_range(syntax, cursor_byte)
			}
		} else {
			// No current range - find one
			find_focus_range(syntax, cursor_byte)
		}
	};

	// Request the focus range change (debounced)
	state.request_focus_range(new_focus_range);
	state.commit_pending();

	// Read current state for rendering
	let effective_range = state.effective_range().cloned();
	let is_animating = state.is_animating();
	let progress = state.animation_progress();
	let prev_focus_range = state.prev_focus_range.clone();
	let has_pending = state.has_pending();
	let is_undim_all = state.is_undim_all();

	let doc_len = editor.buffer().doc().content.len_bytes();

	if animate && is_animating {
		// Use different easing for undim-all (slower, more gradual)
		let eased_progress = if is_undim_all {
			Easing::EaseInOut.apply(progress)
		} else {
			Easing::EaseOut.apply(progress)
		};

		match (&effective_range, &prev_focus_range) {
			(Some(new_range), Some(_prev_range)) => {
				// Transitioning between two focus ranges
				// Animate dim factor from full brightness to target
				let current_dim = 1.0 - (1.0 - dim_factor) * eased_progress;
				editor.style_overlays.dim_outside(
					new_range.clone(),
					current_dim,
					"zenmode",
					doc_len,
				);
			}
			(Some(new_range), None) => {
				// Transitioning from no focus to having a focus range
				// Fade in the dimming: start at full brightness, end at dim_factor
				let current_dim = 1.0 - (1.0 - dim_factor) * eased_progress;
				editor.style_overlays.dim_outside(
					new_range.clone(),
					current_dim,
					"zenmode",
					doc_len,
				);
			}
			(None, Some(prev_range)) => {
				// Transitioning from focus to no focus (e.g., blank line)
				// Fade out the dimming: start at dim_factor, end at full brightness
				let current_dim = dim_factor + (1.0 - dim_factor) * eased_progress;
				editor.style_overlays.dim_outside(
					prev_range.clone(),
					current_dim,
					"zenmode",
					doc_len,
				);
			}
			(None, None) => {
				// No focus before or after - nothing to do
			}
		}

		// Request redraw to continue animation
		editor.needs_redraw = true;
	} else {
		// No animation in progress - apply the current effective state
		if let Some(range) = effective_range {
			editor
				.style_overlays
				.dim_outside(range, dim_factor, "zenmode", doc_len);
		}
	}

	// If there's a pending change, request redraw to check debounce
	if has_pending {
		editor.needs_redraw = true;
	}
}
