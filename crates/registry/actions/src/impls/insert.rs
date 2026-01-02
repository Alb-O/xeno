use crate::{action, insert_with_motion, ActionMode, ActionResult};

action!(insert_mode, { description: "Switch to insert mode", bindings: r#"normal "i""# },
	|_ctx| ActionResult::ModeChange(ActionMode::Insert));

action!(insert_line_start, { description: "Insert at start of line", bindings: r#"normal "I""# },
	|ctx| insert_with_motion(ctx, "line_start"));

action!(insert_line_end, { description: "Insert at end of line", bindings: r#"normal "A""# },
	|ctx| insert_with_motion(ctx, "line_end"));

action!(insert_after, { description: "Insert after cursor", bindings: r#"normal "a""# },
	|ctx| insert_with_motion(ctx, "right"));
