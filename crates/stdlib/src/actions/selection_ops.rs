//! Selection manipulation actions (collapse, flip, select all, etc.).

use tome_base::selection::Selection;
use tome_manifest::actions::{ActionContext, ActionResult};

use crate::action;

action!(collapse_selection, { description: "Collapse selection to cursor" }, handler: collapse_selection);

fn collapse_selection(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		r.anchor = r.head;
	});
	ActionResult::Motion(new_sel)
}

action!(flip_selection, { description: "Flip selection direction" }, handler: flip_selection);

fn flip_selection(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		std::mem::swap(&mut r.anchor, &mut r.head);
	});
	ActionResult::Motion(new_sel)
}

action!(ensure_forward, { description: "Ensure selection is forward" }, handler: ensure_forward);

fn ensure_forward(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		if r.head < r.anchor {
			std::mem::swap(&mut r.anchor, &mut r.head);
		}
	});
	ActionResult::Motion(new_sel)
}

action!(select_line, { description: "Select current line" }, handler: select_line);

fn select_line(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	let count = ctx.count.max(1);
	new_sel.transform_mut(|r| {
		let line = ctx.text.char_to_line(r.head);
		let start = ctx.text.line_to_char(line);
		let end = if line + count < ctx.text.len_lines() {
			ctx.text.line_to_char(line + count)
		} else {
			ctx.text.len_chars()
		};

		if ctx.extend {
			r.head = end;
		} else {
			r.anchor = start;
			r.head = end;
		}
	});
	ActionResult::Motion(new_sel)
}

action!(select_all, { description: "Select all text" }, |ctx| {
	let end = ctx.text.len_chars();
	ActionResult::Motion(Selection::single(0, end))
});

action!(expand_to_line, { description: "Expand selection to cover full lines" }, handler: expand_to_line);

fn expand_to_line(ctx: &ActionContext) -> ActionResult {
	let mut new_sel = ctx.selection.clone();
	new_sel.transform_mut(|r| {
		let start_line = ctx.text.char_to_line(r.min());
		let end_line = ctx.text.char_to_line(r.max());
		r.anchor = ctx.text.line_to_char(start_line);
		r.head = if end_line + 1 < ctx.text.len_lines() {
			ctx.text.line_to_char(end_line + 1)
		} else {
			ctx.text.len_chars()
		};
	});
	ActionResult::Motion(new_sel)
}

action!(remove_primary_selection, { description: "Remove the primary selection" }, handler: remove_primary_selection);

fn remove_primary_selection(ctx: &ActionContext) -> ActionResult {
	if ctx.selection.len() <= 1 {
		return ActionResult::Ok;
	}
	let mut new_sel = ctx.selection.clone();
	new_sel.remove_primary();
	ActionResult::Motion(new_sel)
}

action!(
	remove_selections_except_primary,
	{ description: "Remove all selections except the primary one" },
	|ctx| {
		ActionResult::Motion(Selection::single(
			ctx.selection.primary().anchor,
			ctx.selection.primary().head,
		))
	}
);

action!(
	rotate_selections_forward,
	{ description: "Rotate selections forward" },
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.rotate_forward();
		ActionResult::Motion(new_sel)
	}
);

action!(
	rotate_selections_backward,
	{ description: "Rotate selections backward" },
	|ctx| {
		let mut new_sel = ctx.selection.clone();
		new_sel.rotate_backward();
		ActionResult::Motion(new_sel)
	}
);

action!(
	split_lines,
	{ description: "Split selection into lines" },
	result: ActionResult::SplitLines
);

#[cfg(test)]
mod tests {
	use tome_manifest::actions::ActionArgs;

	use super::*;
	use crate::{Rope, Selection};

	/// Tests line selection with extend mode starting from partial line selection.
	#[test]
	fn test_select_line_extend() {
		let text = Rope::from("line 1\nline 2\nline 3\n");
		let sel = Selection::single(1, 6);

		let ctx = ActionContext {
			text: text.slice(..),
			cursor: 6,
			selection: &sel,
			count: 1,
			extend: true,
			register: None,
			args: ActionArgs::default(),
		};

		let result = select_line(&ctx);
		if let ActionResult::Motion(new_sel) = result {
			let primary = new_sel.primary();
			// If it extends, anchor should stay at 1, head should be start of next line (7)
			assert_eq!(
				primary.anchor, 1,
				"Anchor should be preserved when extending"
			);
			assert_eq!(primary.head, 7, "Head should be at end of line");
		} else {
			panic!("Expected Motion result");
		}
	}

	/// Tests repeated line selection replaces with next line in normal mode.
	#[test]
	fn test_select_line_repeated() {
		let text = Rope::from("line 1\nline 2\nline 3\n");
		let sel = Selection::single(0, 7);

		let ctx = ActionContext {
			text: text.slice(..),
			cursor: 7,
			selection: &sel,
			count: 1,
			extend: false,
			register: None,
			args: ActionArgs::default(),
		};

		let result = select_line(&ctx);
		if let ActionResult::Motion(new_sel) = result {
			let primary = new_sel.primary();
			// Should replace with line 2
			assert_eq!(
				primary.anchor, 7,
				"Anchor should move to start of next line (replace behavior)"
			);
			assert_eq!(primary.head, 14, "Head should move to end of next line");
		} else {
			panic!("Expected Motion result");
		}
	}

	/// Tests line selection with count selects multiple lines.
	#[test]
	fn test_select_line_count() {
		let text = Rope::from("line 1\nline 2\nline 3\n");
		let sel = Selection::point(0);

		let ctx = ActionContext {
			text: text.slice(..),
			cursor: 0,
			selection: &sel,
			count: 2,
			extend: false,
			register: None,
			args: ActionArgs::default(),
		};

		let result = select_line(&ctx);
		if let ActionResult::Motion(new_sel) = result {
			let primary = new_sel.primary();
			assert_eq!(primary.anchor, 0);
			assert_eq!(primary.head, 14, "should select 2 complete lines");
		} else {
			panic!("Expected Motion result");
		}
	}
}
