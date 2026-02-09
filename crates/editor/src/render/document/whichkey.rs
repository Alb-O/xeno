//! Which-key HUD rendering.
//!
//! Displays a popup showing available key continuations when
//! there are pending keys in the input buffer.

use xeno_registry::actions::{BindingMode, find_prefix};
use xeno_registry::db::keymap_registry::ContinuationKind;
use xeno_registry::get_keymap_registry;
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::widgets::keytree::{KeyTree, KeyTreeNode};
use xeno_tui::widgets::{Block, BorderType, Borders, Clear, Padding};

use crate::Editor;
use crate::render::RenderCtx;

impl Editor {
	/// Renders the which-key HUD when there are pending keys.
	pub fn render_whichkey_hud(
		&self,
		frame: &mut xeno_tui::Frame,
		doc_area: Rect,
		ctx: &RenderCtx,
	) {
		let pending_keys = self.buffer().input.pending_keys();
		if pending_keys.is_empty() {
			return;
		}

		let binding_mode = match self.buffer().input.mode() {
			xeno_primitives::Mode::Normal => BindingMode::Normal,
			_ => return,
		};

		let binding = get_keymap_registry();
		let continuations = binding.continuations_with_kind(binding_mode, pending_keys);
		if continuations.is_empty() {
			return;
		}

		let key_strs: Vec<String> = pending_keys.iter().map(|k| k.to_string()).collect();
		let (root, ancestors) = if key_strs.len() <= 1 {
			(key_strs.first().cloned().unwrap_or_default(), vec![])
		} else {
			let root = key_strs[0].clone();
			let mut ancestors = Vec::new();
			let mut prefix_so_far = root.clone();

			for key in &key_strs[1..] {
				prefix_so_far = format!("{prefix_so_far} {key}");
				let desc = find_prefix(binding_mode, &prefix_so_far)
					.map_or(String::new(), |p| p.description.to_string());
				ancestors.push(KeyTreeNode::new(key.clone(), desc));
			}
			(root, ancestors)
		};

		let prefix_key = key_strs.join(" ");
		let root_desc = find_prefix(binding_mode, &key_strs[0]).map(|p| p.description.to_string());

		let children: Vec<KeyTreeNode<'_>> = continuations
			.iter()
			.map(|cont| {
				let key = cont.key.to_string();
				match cont.kind {
					ContinuationKind::Branch => {
						let sub_prefix = if prefix_key.is_empty() {
							key.clone()
						} else {
							format!("{prefix_key} {key}")
						};
						let desc = find_prefix(binding_mode, &sub_prefix)
							.map_or(String::new(), |p| p.description.to_string());
						KeyTreeNode::with_suffix(key, desc, "...")
					}
					ContinuationKind::Leaf => {
						let desc = cont.value.map_or("", |e| {
							if !e.short_desc.is_empty() {
								&e.short_desc
							} else if !e.description.is_empty() {
								&e.description
							} else {
								&e.action_name
							}
						});
						KeyTreeNode::new(key, desc)
					}
				}
			})
			.collect();

		let ancestor_lines = ancestors.len() as u16;
		let content_height = (children.len() as u16 + ancestor_lines + 2).clamp(3, 14);
		let width = 32u16.min(doc_area.width.saturating_sub(4));
		let height = content_height + 2;
		let hud_area = Rect {
			x: doc_area.x + doc_area.width.saturating_sub(width + 2),
			y: doc_area.y + doc_area.height.saturating_sub(height + 2),
			width,
			height,
		};

		let block = Block::default()
			.style(
				Style::default()
					.bg(ctx.theme.colors.popup.bg)
					.fg(ctx.theme.colors.popup.fg),
			)
			.borders(Borders::ALL)
			.border_type(BorderType::Stripe)
			.border_style(Style::default().fg(ctx.theme.colors.semantic.accent))
			.padding(Padding::horizontal(1));

		let inner = block.inner(hud_area);
		frame.render_widget(Clear, hud_area);
		frame.render_widget(block, hud_area);

		let mut tree = KeyTree::new(root, children)
			.ancestors(ancestors)
			.ancestor_style(Style::default().fg(ctx.theme.colors.ui.gutter_fg))
			.key_style(
				Style::default()
					.fg(ctx.theme.colors.semantic.accent)
					.add_modifier(Modifier::BOLD),
			)
			.desc_style(Style::default().fg(ctx.theme.colors.popup.fg))
			.suffix_style(Style::default().fg(ctx.theme.colors.ui.gutter_fg))
			.line_style(Style::default().fg(ctx.theme.colors.ui.gutter_fg));

		if let Some(desc) = root_desc {
			tree = tree.root_desc(desc);
		}

		frame.render_widget(tree, inner);
	}
}
