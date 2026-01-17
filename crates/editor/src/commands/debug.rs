//! Debug commands for observability.

use futures::future::LocalBoxFuture;

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::editor_command;
use crate::info_popup::PopupAnchor;

editor_command!(
	stats,
	{
		aliases: &["editor-stats", "debug-stats"],
		description: "Show editor runtime statistics"
	},
	handler: cmd_stats
);

fn cmd_stats<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let stats = ctx.editor.stats_snapshot();

		// Emit to tracing for log viewer
		stats.emit();

		let content = format!(
			"# Editor Statistics

## Hooks
- Pending: {}
- Scheduled: {}
- Completed: {}

## LSP Sync
- Pending docs: {}
- In-flight: {}
- Full syncs (total/tick): {} / {}
- Incremental syncs (total/tick): {} / {}
- Send errors: {}
- Coalesced: {}
- Snapshot bytes (total/tick): {} / {}",
			stats.hooks_pending,
			stats.hooks_scheduled,
			stats.hooks_completed,
			stats.lsp_pending_docs,
			stats.lsp_in_flight,
			stats.lsp_full_sync,
			stats.lsp_full_sync_tick,
			stats.lsp_incremental_sync,
			stats.lsp_incremental_sync_tick,
			stats.lsp_send_errors,
			stats.lsp_coalesced,
			stats.lsp_snapshot_bytes,
			stats.lsp_snapshot_bytes_tick,
		);

		crate::impls::Editor::open_info_popup(
			ctx.editor,
			content,
			Some("markdown"),
			PopupAnchor::Center,
		);

		Ok(CommandOutcome::Ok)
	})
}
