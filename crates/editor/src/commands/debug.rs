//! Debug commands for observability.

use futures::future::LocalBoxFuture;

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::editor_command;
use crate::info_popup::PopupAnchor;
use xeno_registry::{Capability, GUTTERS, HOOKS, NOTIFICATIONS, STATUSLINE_SEGMENTS};
use xeno_registry::index::get_registry;
use xeno_registry::options::OPTIONS;
use xeno_registry::themes::THEMES;

editor_command!(
	stats,
	{
		aliases: &["editor-stats", "debug-stats"],
		description: "Show editor runtime statistics"
	},
	handler: cmd_stats
);

editor_command!(
	registry,
	{
		aliases: &["reg"],
		description: "List registry items"
	},
	handler: cmd_registry
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

		crate::impls::Editor::open_info_popup(
			ctx.editor,
			content,
			Some("markdown"),
			PopupAnchor::Center,
		);

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
	id: &'static str,
	name: &'static str,
	description: String,
	priority: i16,
	source: xeno_registry::RegistrySource,
	required_caps: &'static [Capability],
	flags: u32,
}

fn cmd_registry<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let (kind, prefix) = parse_registry_args(ctx.args);
		let content = build_registry_report(kind, prefix);
		crate::impls::Editor::open_info_popup(
			ctx.editor,
			content,
			Some("markdown"),
			PopupAnchor::Center,
		);
		Ok(CommandOutcome::Ok)
	})
}

fn parse_registry_args<'a>(args: &'a [&'a str]) -> (Option<RegistryKind>, Option<&'a str>) {
	let mut kind = None;
	let mut prefix = None;

	for arg in args {
		if kind.is_none() && let Some(parsed) = RegistryKind::parse(arg) {
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

fn append_registry_section(
	out: &mut String,
	kind: RegistryKind,
	prefix: Option<&str>,
	show_empty: bool,
) -> usize {
	let mut items = collect_registry_items(kind);
	items.retain(|item| matches_prefix(item, prefix));

	if items.is_empty() && !show_empty {
		return 0;
	}

	items.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));
	let count = items.len();

	out.push_str(&format!("## {} ({})\n", kind.label(), count));
	for item in items {
		out.push_str(&format!(
			"- {} ({}) prio={} src={} caps={} flags={} - {}\n",
			item.id,
			item.name,
			item.priority,
			item.source,
			format_caps(item.required_caps),
			format_flags(item.flags),
			item.description
		));
	}
	out.push('\n');

	count
}

fn collect_registry_items(kind: RegistryKind) -> Vec<RegistryItem> {
	let reg = get_registry();
	match kind {
		RegistryKind::Actions => reg
			.actions
			.base
			.by_id
			.values()
			.copied()
			.map(|def| RegistryItem {
				id: def.meta.id,
				name: def.meta.name,
				description: def.meta.description.to_string(),
				priority: def.meta.priority,
				source: def.meta.source,
				required_caps: def.meta.required_caps,
				flags: def.meta.flags,
			})
			.collect(),
		RegistryKind::Commands => reg
			.commands
			.by_id
			.values()
			.copied()
			.map(|def| RegistryItem {
				id: def.meta.id,
				name: def.meta.name,
				description: def.meta.description.to_string(),
				priority: def.meta.priority,
				source: def.meta.source,
				required_caps: def.meta.required_caps,
				flags: def.meta.flags,
			})
			.collect(),
		RegistryKind::EditorCommands => crate::commands::EDITOR_COMMANDS
			.iter()
			.copied()
			.map(|def| RegistryItem {
				id: def.id,
				name: def.name,
				description: def.description.to_string(),
				priority: def.priority,
				source: def.source,
				required_caps: def.required_caps,
				flags: 0,
			})
			.collect(),
		RegistryKind::Motions => reg
			.motions
			.by_id
			.values()
			.copied()
			.map(|def| RegistryItem {
				id: def.meta.id,
				name: def.meta.name,
				description: def.meta.description.to_string(),
				priority: def.meta.priority,
				source: def.meta.source,
				required_caps: def.meta.required_caps,
				flags: def.meta.flags,
			})
			.collect(),
		RegistryKind::TextObjects => reg
			.text_objects
			.by_id
			.values()
			.copied()
			.map(|def| RegistryItem {
				id: def.meta.id,
				name: def.meta.name,
				description: def.meta.description.to_string(),
				priority: def.meta.priority,
				source: def.meta.source,
				required_caps: def.meta.required_caps,
				flags: def.meta.flags,
			})
			.collect(),
		RegistryKind::Gutters => GUTTERS
			.iter()
			.map(|def| RegistryItem {
				id: def.meta.id,
				name: def.meta.name,
				description: def.meta.description.to_string(),
				priority: def.meta.priority,
				source: def.meta.source,
				required_caps: def.meta.required_caps,
				flags: def.meta.flags,
			})
			.collect(),
		RegistryKind::Hooks => HOOKS
			.iter()
			.copied()
			.map(|def| RegistryItem {
				id: def.meta.id,
				name: def.meta.name,
				description: def.meta.description.to_string(),
				priority: def.meta.priority,
				source: def.meta.source,
				required_caps: def.meta.required_caps,
				flags: def.meta.flags,
			})
			.collect(),
		RegistryKind::Notifications => NOTIFICATIONS
			.iter()
			.copied()
			.map(|def| RegistryItem {
				id: def.id,
				name: def.id,
				description: format!(
					"level={:?}, auto_dismiss={:?}",
					def.level,
					def.auto_dismiss
				),
				priority: 0,
				source: def.source,
				required_caps: &[],
				flags: 0,
			})
			.collect(),
		RegistryKind::Options => OPTIONS
			.iter()
			.map(|def| RegistryItem {
				id: def.meta.id,
				name: def.meta.name,
				description: def.meta.description.to_string(),
				priority: def.meta.priority,
				source: def.meta.source,
				required_caps: def.meta.required_caps,
				flags: def.meta.flags,
			})
			.collect(),
		RegistryKind::Statusline => STATUSLINE_SEGMENTS
			.iter()
			.map(|def| RegistryItem {
				id: def.meta.id,
				name: def.meta.name,
				description: def.meta.description.to_string(),
				priority: def.meta.priority,
				source: def.meta.source,
				required_caps: def.meta.required_caps,
				flags: def.meta.flags,
			})
			.collect(),
		RegistryKind::Themes => THEMES
			.iter()
			.map(|def| RegistryItem {
				id: def.meta.id,
				name: def.meta.name,
				description: def.meta.description.to_string(),
				priority: def.meta.priority,
				source: def.meta.source,
				required_caps: def.meta.required_caps,
				flags: def.meta.flags,
			})
			.collect(),
	}
}

fn matches_prefix(item: &RegistryItem, prefix: Option<&str>) -> bool {
	let Some(prefix) = prefix else {
		return true;
	};
	item.id.starts_with(prefix) || item.name.starts_with(prefix)
}

fn format_caps(caps: &[Capability]) -> String {
	if caps.is_empty() {
		return "[]".to_string();
	}
	use std::fmt::Write;
	let mut out = String::from("[");
	for (i, cap) in caps.iter().enumerate() {
		if i > 0 {
			out.push_str(", ");
		}
		write!(out, "{cap:?}").unwrap();
	}
	out.push(']');
	out
}

fn format_flags(flags: u32) -> String {
	format!("0x{flags:08x}")
}
