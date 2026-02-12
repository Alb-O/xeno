use xeno_editor::Editor;
use xeno_editor::render::RenderCtx;
use xeno_editor::ui::PanelRenderTarget;
use xeno_editor::ui::ids::UTILITY_PANEL_ID;
use xeno_registry::Mode;
use xeno_registry::actions::BindingMode;
use xeno_registry::db::keymap_registry::ContinuationKind;
use xeno_tui::layout::{Position, Rect};
use xeno_tui::style::{Modifier, Style};
use xeno_tui::widgets::keytree::{KeyTree, KeyTreeNode};
use xeno_tui::widgets::{Block, Paragraph};

fn utility_whichkey_data(ed: &Editor) -> Option<(String, Option<String>, Vec<KeyTreeNode<'static>>)> {
	let pending_keys = ed.buffer().input.pending_keys();
	if pending_keys.is_empty() {
		return None;
	}

	let binding_mode = match ed.buffer().input.mode() {
		Mode::Normal => BindingMode::Normal,
		_ => return None,
	};

	let registry = ed.effective_keymap();
	let continuations = registry.continuations_with_kind(binding_mode, pending_keys);
	if continuations.is_empty() {
		return None;
	}

	let key_strs: Vec<String> = pending_keys.iter().map(|k| k.to_string()).collect();
	let root = key_strs.first().cloned().unwrap_or_default();
	let prefix_key = key_strs.join(" ");
	let root_desc = xeno_registry::actions::find_prefix(binding_mode, &root).map(|prefix| prefix.description.to_string());

	let children: Vec<KeyTreeNode<'static>> = continuations
		.iter()
		.map(|cont| {
			let key = cont.key.to_string();
			match cont.kind {
				ContinuationKind::Branch => {
					let sub_prefix = if prefix_key.is_empty() { key.clone() } else { format!("{prefix_key} {key}") };
					let desc = xeno_registry::actions::find_prefix(binding_mode, &sub_prefix).map_or(String::new(), |p| p.description.to_string());
					KeyTreeNode::with_suffix(key, desc, "...")
				}
				ContinuationKind::Leaf => {
					let desc = cont.value.map_or("", |entry| {
						if !entry.short_desc.is_empty() {
							&entry.short_desc
						} else if !entry.description.is_empty() {
							&entry.description
						} else {
							&entry.action_name
						}
					});
					KeyTreeNode::new(key, desc.to_string())
				}
			}
		})
		.collect();

	Some((root, root_desc, children))
}

fn render_utility_panel(ed: &mut Editor, frame: &mut xeno_tui::Frame, area: Rect, focused: bool, ctx: &RenderCtx) {
	let theme = &ctx.theme;
	let bg = if focused { theme.colors.ui.selection_bg } else { theme.colors.popup.bg };
	let fg = if focused { theme.colors.ui.selection_fg } else { theme.colors.popup.fg };

	let block = Block::default().style(Style::default().bg(bg).fg(fg));
	let inner = block.inner(area);
	frame.render_widget(block, area);

	if inner.width == 0 || inner.height == 0 {
		return;
	}

	if ed.overlay_interaction().is_open() {
		crate::layers::modal_overlays::render_utility_panel_overlay(ed, frame, area, ctx);
		return;
	}

	if let Some((root, root_desc, children)) = utility_whichkey_data(ed) {
		let mut tree = KeyTree::new(root, children)
			.key_style(Style::default().fg(theme.colors.semantic.accent).add_modifier(Modifier::BOLD))
			.desc_style(Style::default().fg(fg).bg(bg))
			.suffix_style(Style::default().fg(theme.colors.ui.gutter_fg).bg(bg))
			.line_style(Style::default().fg(theme.colors.ui.gutter_fg).bg(bg));
		if let Some(desc) = root_desc {
			tree = tree.root_desc(desc);
		}
		frame.render_widget(tree, inner);
		return;
	}

	let hint = "Utility panel: Ctrl-U toggle, Esc close";
	frame.render_widget(Paragraph::new(hint).style(Style::default().fg(fg).bg(bg)), inner);
}

pub fn render_panels(editor: &mut Editor, frame: &mut xeno_tui::Frame, plan: &[PanelRenderTarget], ctx: &RenderCtx) -> Option<Position> {
	for target in plan {
		if target.id == UTILITY_PANEL_ID {
			render_utility_panel(editor, frame, target.area, target.focused, ctx);
		}
	}
	None
}
