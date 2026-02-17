//! Debug commands for observability.

use xeno_primitives::BoxFutureLocal;
use xeno_registry::index::{all_actions, all_commands, all_motions, all_text_objects};
use xeno_registry::options::OPTIONS;
use xeno_registry::themes::THEMES;
use xeno_registry::{GUTTERS, HOOKS, NOTIFICATIONS, RegistryMetadata, STATUSLINE_SEGMENTS};

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::editor_command;
use crate::info_popup::PopupAnchor;

editor_command!(
	stats,
	{
		keys: &["editor-stats", "debug-stats"],
		description: "Show editor runtime statistics"
	},
	handler: cmd_stats
);

editor_command!(
	registry,
	{
		keys: &["reg"],
		description: "List registry items"
	},
	handler: cmd_registry
);

editor_command!(
	files,
	{
		keys: &["fp"],
		description: "Open file picker"
	},
	handler: cmd_files
);

fn cmd_files<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.open_file_picker();
		Ok(CommandOutcome::Ok)
	})
}

fn cmd_stats<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let stats = ctx.editor.stats_snapshot();

		// Emit to tracing for log viewer
		stats.emit();

		let nu_in_flight = match &stats.nu.hook_in_flight {
			Some((epoch, seq, hook)) => format!("{epoch}:{seq}:{hook}"),
			None => "none".to_string(),
		};
		let nu_script = stats.nu.script_path.as_deref().unwrap_or("none");

		let content = format!(
			"# Editor Statistics

## Nu
- Runtime: loaded={} executor={} script={}
- Hooks: phase={} queued={} in_flight={} runtime_work_queue={} dropped={} failed={} epoch={} next_seq={}
- Macros: depth={}

## Work Scheduler
- Pending (current/tick): {} / {}
- Scheduled: {}
- Completed (total/tick): {} / {}

## LSP Sync
- Pending docs: {}
- In-flight: {}
- Full syncs (total/tick): {} / {}
- Incremental syncs (total/tick): {} / {}
- Send errors: {}
- Coalesced: {}
- Snapshot bytes (total/tick): {} / {}",
			stats.nu.runtime_loaded,
			stats.nu.executor_alive,
			nu_script,
			stats.nu.hook_phase,
			stats.nu.hook_queue_len,
			nu_in_flight,
			stats.nu.runtime_work_queue_len,
			stats.nu.hook_dropped_total,
			stats.nu.hook_failed_total,
			stats.nu.runtime_epoch,
			stats.nu.hook_eval_seq_next,
			stats.nu.macro_depth,
			stats.hooks_pending,
			stats.hooks_pending_tick,
			stats.hooks_scheduled,
			stats.hooks_completed,
			stats.hooks_completed_tick,
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

		crate::Editor::open_info_popup(ctx.editor, content, Some("markdown"), PopupAnchor::Center);

		Ok(CommandOutcome::Ok)
	})
}

#[derive(Debug, Clone, Copy)]
enum RegistryKind {
	Actions,
	Commands,
	EditorCommands,
	Motions,
	TextObjects,
	Gutters,
	Hooks,
	Notifications,
	Options,
	Statusline,
	Themes,
}

impl RegistryKind {
	fn parse(value: &str) -> Option<Self> {
		match value {
			"actions" | "action" => Some(Self::Actions),
			"commands" | "command" => Some(Self::Commands),
			"editor_commands" | "editor-command" | "editor" => Some(Self::EditorCommands),
			"motions" | "motion" => Some(Self::Motions),
			"text_objects" | "text-objects" | "textobj" => Some(Self::TextObjects),
			"gutters" | "gutter" => Some(Self::Gutters),
			"hooks" | "hook" => Some(Self::Hooks),
			"notifications" | "notification" => Some(Self::Notifications),
			"options" | "option" => Some(Self::Options),
			"statusline" | "status" => Some(Self::Statusline),
			"themes" | "theme" => Some(Self::Themes),
			_ => None,
		}
	}

	fn label(self) -> &'static str {
		match self {
			Self::Actions => "actions",
			Self::Commands => "commands",
			Self::EditorCommands => "editor_commands",
			Self::Motions => "motions",
			Self::TextObjects => "text_objects",
			Self::Gutters => "gutters",
			Self::Hooks => "hooks",
			Self::Notifications => "notifications",
			Self::Options => "options",
			Self::Statusline => "statusline",
			Self::Themes => "themes",
		}
	}
}

#[derive(Debug, Clone)]
struct RegistryItem {
	id: String,
	name: String,
	description: String,
	priority: i16,
	source: xeno_registry::RegistrySource,
	required_caps: String,
	flags: u32,
}

fn cmd_registry<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let (kind, prefix) = parse_registry_args(ctx.args);
		let content = build_registry_report(kind, prefix);
		crate::Editor::open_info_popup(ctx.editor, content, Some("markdown"), PopupAnchor::Center);
		Ok(CommandOutcome::Ok)
	})
}

fn parse_registry_args<'a>(args: &'a [&'a str]) -> (Option<RegistryKind>, Option<&'a str>) {
	let mut kind = None;
	let mut prefix = None;

	for arg in args {
		if kind.is_none()
			&& let Some(parsed) = RegistryKind::parse(arg)
		{
			kind = Some(parsed);
			continue;
		}
		if prefix.is_none() {
			prefix = Some(*arg);
		}
	}

	(kind, prefix)
}

fn build_registry_report(kind: Option<RegistryKind>, prefix: Option<&str>) -> String {
	let mut out = String::from("# Registry\n\n");
	let mut sections = 0;
	let show_empty = kind.is_some();

	let kinds: Vec<RegistryKind> = match kind {
		Some(kind) => vec![kind],
		None => vec![
			RegistryKind::Actions,
			RegistryKind::Commands,
			RegistryKind::EditorCommands,
			RegistryKind::Motions,
			RegistryKind::TextObjects,
			RegistryKind::Gutters,
			RegistryKind::Hooks,
			RegistryKind::Notifications,
			RegistryKind::Options,
			RegistryKind::Statusline,
			RegistryKind::Themes,
		],
	};

	for kind in kinds {
		let count = append_registry_section(&mut out, kind, prefix, show_empty);
		if count > 0 {
			sections += 1;
		}
	}

	if sections == 0 {
		out.push_str("No registry entries matched.\n");
	}

	out
}

fn append_registry_section(out: &mut String, kind: RegistryKind, prefix: Option<&str>, show_empty: bool) -> usize {
	let mut items = collect_registry_items(kind);
	items.retain(|item| matches_prefix(item, prefix));

	if items.is_empty() && !show_empty {
		return 0;
	}

	items.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(&b.id)));
	let count = items.len();

	out.push_str(&format!("## {} ({})\n", kind.label(), count));
	for item in items {
		out.push_str(&format!(
			"- {} ({}) prio={} src={} caps={} flags={} - {}\n",
			item.id,
			item.name,
			item.priority,
			item.source,
			item.required_caps,
			format_flags(item.flags),
			item.description
		));
	}
	out.push('\n');

	count
}

fn registry_item_from_ref<T, Id>(def: &xeno_registry::core::RegistryRef<T, Id>) -> RegistryItem
where
	T: xeno_registry::core::RuntimeEntry,
	Id: xeno_registry::core::DenseId,
{
	RegistryItem {
		id: def.id_str().to_string(),
		name: def.name_str().to_string(),
		description: def.description_str().to_string(),
		priority: def.priority(),
		source: def.source(),
		required_caps: format_caps(def.required_caps()),
		flags: def.flags(),
	}
}

fn collect_registry_items(kind: RegistryKind) -> Vec<RegistryItem> {
	match kind {
		RegistryKind::Actions => all_actions().iter().map(registry_item_from_ref).collect(),
		RegistryKind::Commands => all_commands().iter().map(registry_item_from_ref).collect(),
		RegistryKind::EditorCommands => crate::commands::EDITOR_COMMANDS
			.iter()
			.copied()
			.map(|def| RegistryItem {
				id: def.id.to_string(),
				name: def.name.to_string(),
				description: def.description.to_string(),
				priority: def.priority,
				source: def.source,
				required_caps: format!("{:?}", xeno_registry::CapabilitySet::from_iter(def.required_caps.iter().cloned())),
				flags: 0,
			})
			.collect(),
		RegistryKind::Motions => all_motions().iter().map(registry_item_from_ref).collect(),
		RegistryKind::TextObjects => all_text_objects().iter().map(registry_item_from_ref).collect(),
		RegistryKind::Gutters => GUTTERS.snapshot_guard().iter_refs().map(|r| registry_item_from_ref(&r)).collect(),
		RegistryKind::Hooks => HOOKS.snapshot_guard().iter_refs().map(|r| registry_item_from_ref(&r)).collect(),
		RegistryKind::Notifications => NOTIFICATIONS
			.snapshot_guard()
			.iter_refs()
			.map(|def| RegistryItem {
				id: def.id_str().to_string(),
				name: def.id_str().to_string(),
				description: format!("level={:?}, auto_dismiss={:?}", def.level, def.auto_dismiss),
				priority: 0,
				source: def.source(),
				required_caps: "[]".to_string(),
				flags: 0,
			})
			.collect(),
		RegistryKind::Options => OPTIONS.snapshot_guard().iter_refs().map(|r| registry_item_from_ref(&r)).collect(),
		RegistryKind::Statusline => STATUSLINE_SEGMENTS.snapshot_guard().iter_refs().map(|r| registry_item_from_ref(&r)).collect(),
		RegistryKind::Themes => THEMES.snapshot_guard().iter_refs().map(|r| registry_item_from_ref(&r)).collect(),
	}
}

fn matches_prefix(item: &RegistryItem, prefix: Option<&str>) -> bool {
	let Some(prefix) = prefix else {
		return true;
	};
	item.id.starts_with(prefix) || item.name.starts_with(prefix)
}

fn format_caps(caps: xeno_registry::CapabilitySet) -> String {
	format!("{caps:?}")
}

fn format_flags(flags: u32) -> String {
	format!("0x{flags:08x}")
}
