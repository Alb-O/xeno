//! Commands for the zen mode extension.

use futures::future::LocalBoxFuture;
use evildoer_api::editor::Editor;
use evildoer_manifest::{CommandContext, CommandError, CommandOutcome};
use evildoer_stdlib::{NotifyINFOExt, command};

use crate::zenmode::ZenmodeState;

command!(zenmode, {
	aliases: &["zen", "focus"],
	description: "Toggle zen/focus mode for syntax highlighting"
}, handler: cmd_zenmode);

fn cmd_zenmode<'a>(
	ctx: &'a mut CommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let editor = ctx.require_editor_mut();
		if let Some(state) = editor.extensions.get_mut::<ZenmodeState>() {
			state.toggle();
			let status = if state.enabled { "enabled" } else { "disabled" };
			ctx.info(&format!("Zen mode {}", status));
			Ok(CommandOutcome::Ok)
		} else {
			Err(CommandError::Failed(
				"Zenmode extension not loaded".to_string(),
			))
		}
	})
}

trait CommandContextExt {
	fn require_editor_mut(&mut self) -> &mut Editor;
}

impl<'a> CommandContextExt for CommandContext<'a> {
	fn require_editor_mut(&mut self) -> &mut Editor {
		// SAFETY: We know that in evildoer-term, EditorOps is implemented by Editor
		unsafe { &mut *(self.editor as *mut dyn evildoer_manifest::EditorOps as *mut Editor) }
	}
}
