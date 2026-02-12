use xeno_primitives::KeyCode;
use xeno_registry::actions::BindingMode;
use xeno_registry::db::keymap_registry::ContinuationKind;
use xeno_registry::themes::Theme;
use xeno_tui::Frame;
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::widgets::keytree::{KeyTree, KeyTreeNode};
use xeno_tui::widgets::{Block, Paragraph};

use crate::impls::Editor;
use crate::ui::UiRequest;
use crate::ui::dock::DockSlot;
use crate::ui::ids::UTILITY_PANEL_ID;
use crate::ui::keymap::UiKeyChord;
use crate::ui::panel::{EventResult, Panel, PanelInitContext, UiEvent};

#[derive(Default)]
pub struct UtilityPanel;

impl UtilityPanel {
	fn whichkey_data(ed: &Editor) -> Option<(String, Option<String>, Vec<KeyTreeNode<'static>>)> {
		let pending_keys = ed.buffer().input.pending_keys();
		if pending_keys.is_empty() {
			return None;
		}

		let binding_mode = match ed.buffer().input.mode() {
			xeno_primitives::Mode::Normal => BindingMode::Normal,
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

	pub fn whichkey_desired_height(ed: &Editor) -> Option<u16> {
		let (_, _, children) = Self::whichkey_data(ed)?;
		Some((children.len() as u16 + 3).clamp(4, 10))
	}
}

impl Panel for UtilityPanel {
	fn id(&self) -> &str {
		UTILITY_PANEL_ID
	}

	fn default_slot(&self) -> DockSlot {
		DockSlot::Bottom
	}

	fn on_register(&mut self, ctx: PanelInitContext<'_>) {
		ctx.keybindings
			.register_global(UiKeyChord::ctrl_char('u'), 100, vec![UiRequest::TogglePanel(UTILITY_PANEL_ID.to_string())]);
	}

	fn handle_event(&mut self, event: UiEvent, _editor: &mut Editor, focused: bool) -> EventResult {
		match event {
			UiEvent::Key(key) if focused && key.code == KeyCode::Esc => {
				EventResult::consumed().with_request(UiRequest::ClosePanel(UTILITY_PANEL_ID.to_string()))
			}
			_ => EventResult::not_consumed(),
		}
	}

	fn render(&mut self, frame: &mut Frame<'_>, area: Rect, _editor: &mut Editor, focused: bool, theme: &Theme) -> Option<crate::ui::panel::CursorRequest> {
		let bg = if focused { theme.colors.ui.selection_bg } else { theme.colors.popup.bg };
		let fg = if focused { theme.colors.ui.selection_fg } else { theme.colors.popup.fg };

		let block = Block::default().style(Style::default().bg(bg).fg(fg));
		let inner = block.inner(area);
		frame.render_widget(block, area);

		if inner.width > 0 && inner.height > 0 {
			if _editor.state.overlay_system.interaction.is_open() {
				let ctx = _editor.render_ctx();
				crate::ui::layers::modal_overlays::render(_editor, frame, area, &ctx);

				if let Some(active) = _editor.state.overlay_system.interaction.active.as_ref()
					&& matches!(active.controller.name(), "CommandPalette" | "FilePicker")
				{
					let input_rect = active
						.session
						.panes
						.iter()
						.find(|pane| pane.role == crate::overlay::WindowRole::Input)
						.map(|pane| pane.rect);

					if let Some(input_rect) = input_rect {
						let panel_top = area.y;
						let menu_bottom = input_rect.y;
						if panel_top < menu_bottom {
							let completion_state = _editor.overlays().get::<crate::completion::CompletionState>();
							let visible_rows = completion_state
								.filter(|state| state.active)
								.map_or(0u16, |state| state.visible_range().len() as u16);
							let available_rows = menu_bottom.saturating_sub(panel_top);
							let menu_height = visible_rows.min(available_rows);

							if menu_height > 0 {
								let menu_y = menu_bottom.saturating_sub(menu_height);
								let menu_rect = Rect::new(input_rect.x, menu_y, input_rect.width, menu_height);
								frame.render_widget(_editor.render_completion_menu_with_limit(menu_rect, menu_height as usize), menu_rect);
							}
						}
					}
				}
			} else if let Some((root, root_desc, children)) = Self::whichkey_data(_editor) {
				let mut tree = KeyTree::new(root, children)
					.key_style(Style::default().fg(theme.colors.semantic.accent).add_modifier(Modifier::BOLD))
					.desc_style(Style::default().fg(fg).bg(bg))
					.suffix_style(Style::default().fg(theme.colors.ui.gutter_fg).bg(bg))
					.line_style(Style::default().fg(theme.colors.ui.gutter_fg).bg(bg));
				if let Some(desc) = root_desc {
					tree = tree.root_desc(desc);
				}
				frame.render_widget(tree, inner);
			} else {
				let hint = "Utility panel: Ctrl-U toggle, Esc close";
				frame.render_widget(Paragraph::new(hint).style(Style::default().fg(fg).bg(bg)), inner);
			}
		}

		None
	}
}
