//! Debug commands for testing info popups.

use futures::future::LocalBoxFuture;

use crate::{CommandContext, CommandError, CommandOutcome, command};

command!(
	test_popup,
	{ aliases: &["tp"], description: "Show a test info popup with markdown content" },
	handler: test_popup
);

command!(
	test_popup_rust,
	{ aliases: &["tpr"], description: "Show a test info popup with Rust code" },
	handler: test_popup_rust
);

command!(
	close_popups,
	{ aliases: &["cp"], description: "Close all info popups" },
	handler: close_popups
);

pub fn test_popup<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let content = r#"# Function Documentation

Moves the cursor **left** by one character.

## Parameters

- `count`: Number of characters to move (default: 1)

## Example

```rust
fn move_left(ctx: &mut Context) {
    let count = ctx.count.unwrap_or(1);
    ctx.cursor.move_by(-count);
}
```

## See Also

- `move_right` - Move cursor right
- `move_up` - Move cursor up"#;

		ctx.editor.open_info_popup(content, Some("markdown"));
		Ok(CommandOutcome::Ok)
	})
}

pub fn test_popup_rust<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let content = r#"/// Opens an info popup with the given content.
///
/// The popup is positioned relative to the anchor point.
pub fn open_info_popup(
    &mut self,
    content: String,
    file_type: Option<&str>,
    anchor: PopupAnchor,
) -> Option<InfoPopupId> {
    let rect = compute_popup_rect(
        anchor,
        content_width,
        content_height,
        screen_width,
        screen_height,
    );

    let buffer_id = self.buffers.create_scratch();
    buffer.set_readonly_override(Some(true));

    Some(popup_id)
}"#;

		ctx.editor.open_info_popup(content, Some("rust"));
		Ok(CommandOutcome::Ok)
	})
}

pub fn close_popups<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.close_all_info_popups();
		Ok(CommandOutcome::Ok)
	})
}
