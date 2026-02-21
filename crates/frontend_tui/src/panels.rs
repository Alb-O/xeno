use xeno_editor::{Editor, PanelRenderTarget, UTILITY_PANEL_ID};
use xeno_tui::layout::{Position, Rect};
use xeno_tui::style::{Modifier, Style};
use xeno_tui::widgets::keytree::{KeyTree, KeyTreeNode};
use xeno_tui::widgets::{Block, Paragraph};

fn render_utility_panel(ed: &mut Editor, frame: &mut xeno_tui::Frame, area: Rect, focused: bool) {
	let theme = &ed.config().theme;
	let bg = if focused { theme.colors.ui.selection_bg } else { theme.colors.popup.bg };
	let fg = if focused { theme.colors.ui.selection_fg } else { theme.colors.popup.fg };

	let block = Block::default().style(Style::default().bg(bg.into()).fg(fg.into()));
	let inner = block.inner(area);
	frame.render_widget(block, area);

	if inner.width == 0 || inner.height == 0 {
		return;
	}

	if ed.overlay_kind().is_some() {
		crate::layers::modal_overlays::render_utility_panel_overlay(ed, frame, area);
		return;
	}

	if let Some(plan) = ed.whichkey_render_plan() {
		let root = plan.root;
		let root_description = plan.root_description;
		let children: Vec<KeyTreeNode<'static>> = plan
			.entries
			.into_iter()
			.map(|entry| {
				if entry.is_branch {
					KeyTreeNode::with_suffix(entry.key, entry.description, "...")
				} else {
					KeyTreeNode::new(entry.key, entry.description)
				}
			})
			.collect();

		let mut tree = KeyTree::new(root, children)
			.key_style(Style::default().fg(theme.colors.semantic.accent.into()).add_modifier(Modifier::BOLD))
			.desc_style(Style::default().fg(fg.into()).bg(bg.into()))
			.suffix_style(Style::default().fg(theme.colors.ui.gutter_fg.into()).bg(bg.into()))
			.line_style(Style::default().fg(theme.colors.ui.gutter_fg.into()).bg(bg.into()));
		if let Some(desc) = root_description {
			tree = tree.root_desc(desc);
		}
		frame.render_widget(tree, inner);
		return;
	}

	let hint = "Utility panel: Ctrl-U toggle, Esc close";
	frame.render_widget(Paragraph::new(hint).style(Style::default().fg(fg.into()).bg(bg.into())), inner);
}

pub fn render_panels(editor: &mut Editor, frame: &mut xeno_tui::Frame, plan: &[PanelRenderTarget]) -> Option<Position> {
	for target in plan {
		if target.id == UTILITY_PANEL_ID {
			render_utility_panel(editor, frame, target.area.into(), target.focused);
		}
	}
	None
}
