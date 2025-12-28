//! Motion actions that wrap [`MotionDef`](tome_manifest::MotionDef) primitives.

use linkme::distributed_slice;
use tome_base::key::{Key, SpecialKey};
use tome_base::range::Range;
use tome_base::selection::Selection;
use tome_manifest::actions::{ActionContext, ActionDef, ActionResult};
use tome_manifest::keybindings::{BindingMode, KEYBINDINGS_NORMAL, KeyBindingDef};
use tome_manifest::{ACTIONS, find_motion};

/// Cursor movement - moves cursor (and all cursors) without creating new selections unless extending.
fn cursor_move_action(ctx: &ActionContext, motion_name: &str) -> ActionResult {
	let motion = match find_motion(motion_name) {
		Some(m) => m,
		None => return ActionResult::Error(format!("Unknown motion: {}", motion_name)),
	};

	let primary_index = ctx.selection.primary_index();

	// Move every selection head; when not extending, collapse to points at the new head.
	let new_ranges: Vec<Range> = ctx
		.selection
		.ranges()
		.iter()
		.map(|range| {
			let seed = if ctx.extend {
				*range
			} else {
				Range::point(range.head)
			};
			let moved = (motion.handler)(ctx.text, seed, ctx.count, ctx.extend);
			if ctx.extend {
				moved
			} else {
				Range::point(moved.head)
			}
		})
		.collect();

	ActionResult::Motion(Selection::from_vec(new_ranges, primary_index))
}

/// Selection-creating motion - creates new selection from old cursor to new position.
fn selection_motion_action(ctx: &ActionContext, motion_name: &str) -> ActionResult {
	let motion = match find_motion(motion_name) {
		Some(m) => m,
		None => return ActionResult::Error(format!("Unknown motion: {}", motion_name)),
	};

	// For selection-creating motions, we create a selection from cursor to new position
	if ctx.extend {
		// Extend each selection from its anchor using the detached cursor for the primary head
		let primary_index = ctx.selection.primary_index();
		let new_ranges: Vec<Range> = ctx
			.selection
			.ranges()
			.iter()
			.enumerate()
			.map(|(i, range)| {
				let seed = if i == primary_index {
					Range::new(range.anchor, ctx.cursor)
				} else {
					*range
				};
				(motion.handler)(ctx.text, seed, ctx.count, true)
			})
			.collect();
		ActionResult::Motion(Selection::from_vec(new_ranges, primary_index))
	} else {
		// Otherwise start fresh from cursor
		let current_range = Range::point(ctx.cursor);
		let new_range = (motion.handler)(ctx.text, current_range, ctx.count, false);
		ActionResult::Motion(Selection::single(new_range.anchor, new_range.head))
	}
}

macro_rules! cursor_action {
	($name:ident, $motion:expr, $desc:expr) => {
		paste::paste! {
			fn [<handler_ $name>](ctx: &ActionContext) -> ActionResult {
				cursor_move_action(ctx, $motion)
			}

			#[distributed_slice(ACTIONS)]
			static [<ACTION_ $name:upper>]: ActionDef = ActionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: &[],
				description: $desc,
				handler: [<handler_ $name>],
				priority: 0,
				source: tome_manifest::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				required_caps: &[],
				flags: tome_manifest::flags::NONE,
			};
		}
	};
}

macro_rules! bound_cursor_action {
	($name:ident, motion: $motion:expr, key: $key:expr, description: $desc:expr) => {
		paste::paste! {
			fn [<handler_ $name>](ctx: &ActionContext) -> ActionResult {
				cursor_move_action(ctx, $motion)
			}

			#[distributed_slice(ACTIONS)]
			static [<ACTION_ $name:upper>]: ActionDef = ActionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: &[],
				description: $desc,
				handler: [<handler_ $name>],
				priority: 0,
				source: tome_manifest::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				required_caps: &[],
				flags: tome_manifest::flags::NONE,
			};

			#[distributed_slice(KEYBINDINGS_NORMAL)]
			static [<KB_ $name:upper>]: KeyBindingDef = KeyBindingDef {
				mode: BindingMode::Normal,
				key: $key,
				action: stringify!($name),
				priority: 100,
			};
		}
	};

	// With one alt key
	($name:ident, motion: $motion:expr, key: $key:expr, alt_keys: [$alt1:expr], description: $desc:expr) => {
		bound_cursor_action!($name, motion: $motion, key: $key, description: $desc);
		paste::paste! {
			#[distributed_slice(KEYBINDINGS_NORMAL)]
			static [<KB_ $name:upper _ALT1>]: KeyBindingDef = KeyBindingDef {
				mode: BindingMode::Normal,
				key: $alt1,
				action: stringify!($name),
				priority: 100,
			};
		}
	};

	($name:ident, motion: $motion:expr, key: $key:expr, alt_keys: [$alt1:expr, $alt2:expr], description: $desc:expr) => {
		bound_cursor_action!($name, motion: $motion, key: $key, alt_keys: [$alt1], description: $desc);
		paste::paste! {
			#[distributed_slice(KEYBINDINGS_NORMAL)]
			static [<KB_ $name:upper _ALT2>]: KeyBindingDef = KeyBindingDef {
				mode: BindingMode::Normal,
				key: $alt2,
				action: stringify!($name),
				priority: 100,
			};
		}
	};
}

macro_rules! bound_selection_action {
	($name:ident, motion: $motion:expr, key: $key:expr, description: $desc:expr) => {
		paste::paste! {
			fn [<handler_ $name>](ctx: &ActionContext) -> ActionResult {
				selection_motion_action(ctx, $motion)
			}

			#[distributed_slice(ACTIONS)]
			static [<ACTION_ $name:upper>]: ActionDef = ActionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: &[],
				description: $desc,
				handler: [<handler_ $name>],
				priority: 0,
				source: tome_manifest::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
				required_caps: &[],
				flags: tome_manifest::flags::NONE,
			};

			#[distributed_slice(KEYBINDINGS_NORMAL)]
			static [<KB_ $name:upper>]: KeyBindingDef = KeyBindingDef {
				mode: BindingMode::Normal,
				key: $key,
				action: stringify!($name),
				priority: 100,
			};
		}
	};

	($name:ident, motion: $motion:expr, key: $key:expr, alt_keys: [$alt1:expr], description: $desc:expr) => {
		bound_selection_action!($name, motion: $motion, key: $key, description: $desc);
		paste::paste! {
			#[distributed_slice(KEYBINDINGS_NORMAL)]
			static [<KB_ $name:upper _ALT1>]: KeyBindingDef = KeyBindingDef {
				mode: BindingMode::Normal,
				key: $alt1,
				action: stringify!($name),
				priority: 100,
			};
		}
	};
}

bound_cursor_action!(
	move_left,
	motion: "move_left",
	key: Key::char('h'),
	alt_keys: [Key::special(SpecialKey::Left)],
	description: "Move left"
);

bound_cursor_action!(
	move_right,
	motion: "move_right",
	key: Key::char('l'),
	alt_keys: [Key::special(SpecialKey::Right)],
	description: "Move right"
);

cursor_action!(move_up, "move_up", "Move up");
cursor_action!(move_down, "move_down", "Move down");

bound_cursor_action!(
	move_line_start,
	motion: "line_start",
	key: Key::char('0'),
	alt_keys: [Key::special(SpecialKey::Home), Key::alt('h')],
	description: "Move to line start"
);

bound_cursor_action!(
	move_line_end,
	motion: "line_end",
	key: Key::char('$'),
	alt_keys: [Key::special(SpecialKey::End), Key::alt('l')],
	description: "Move to line end"
);

bound_cursor_action!(
	move_first_nonblank,
	motion: "first_nonwhitespace",
	key: Key::char('^'),
	description: "Move to first non-blank"
);

bound_cursor_action!(
	document_start,
	motion: "document_start",
	key: Key::special(SpecialKey::Home).with_ctrl(),
	description: "Move to document start"
);

bound_cursor_action!(
	document_end,
	motion: "document_end",
	key: Key::char('G'),
	alt_keys: [Key::special(SpecialKey::End).with_ctrl()],
	description: "Move to document end"
);

// Selection-creating motions - create selections
bound_selection_action!(
	next_word_start,
	motion: "next_word_start",
	key: Key::char('w'),
	description: "Move to next word start"
);

bound_selection_action!(
	next_word_end,
	motion: "next_word_end",
	key: Key::char('e'),
	description: "Move to next word end"
);

bound_selection_action!(
	prev_word_start,
	motion: "prev_word_start",
	key: Key::char('b'),
	description: "Move to previous word start"
);

bound_selection_action!(
	prev_word_end,
	motion: "prev_word_end",
	key: Key::alt('e'),
	description: "Move to previous word end"
);

bound_selection_action!(
	next_long_word_start,
	motion: "next_long_word_start",
	key: Key::char('W'),
	alt_keys: [Key::alt('w')],
	description: "Move to next WORD start"
);

bound_selection_action!(
	next_long_word_end,
	motion: "next_long_word_end",
	key: Key::char('E'),
	description: "Move to next WORD end"
);

bound_selection_action!(
	prev_long_word_start,
	motion: "prev_long_word_start",
	key: Key::char('B'),
	alt_keys: [Key::alt('b')],
	description: "Move to previous WORD start"
);
