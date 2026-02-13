use xeno_primitives::KeyCode;
use xeno_registry::actions::BindingMode;
use xeno_registry::db::keymap_registry::ContinuationKind;

use crate::Editor;
use crate::ui::UiRequest;
use crate::ui::dock::DockSlot;
use crate::ui::ids::UTILITY_PANEL_ID;
use crate::ui::keymap::UiKeyChord;
use crate::ui::panel::{EventResult, Panel, PanelInitContext, UiEvent};

#[derive(Default)]
pub struct UtilityPanel;

/// Data-only entry for which-key continuation rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtilityWhichKeyEntry {
	pub key: String,
	pub description: String,
	pub is_branch: bool,
}

/// Data-only which-key render plan for the utility panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtilityWhichKeyPlan {
	pub root: String,
	pub root_description: Option<String>,
	pub entries: Vec<UtilityWhichKeyEntry>,
}

impl UtilityPanel {
	pub fn whichkey_desired_height(ed: &Editor) -> Option<u16> {
		let plan = Self::whichkey_render_plan(ed)?;
		Some((plan.entries.len() as u16 + 3).clamp(4, 10))
	}

	/// Returns data-only which-key continuation content for rendering.
	pub fn whichkey_render_plan(ed: &Editor) -> Option<UtilityWhichKeyPlan> {
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
		let root_description = xeno_registry::actions::find_prefix(binding_mode, &root).map(|prefix| prefix.description.to_string());

		let entries = continuations
			.iter()
			.map(|cont| {
				let key = cont.key.to_string();
				match cont.kind {
					ContinuationKind::Branch => {
						let sub_prefix = if prefix_key.is_empty() { key.clone() } else { format!("{prefix_key} {key}") };
						let description =
							xeno_registry::actions::find_prefix(binding_mode, &sub_prefix).map_or_else(String::new, |prefix| prefix.description.to_string());
						UtilityWhichKeyEntry {
							key,
							description,
							is_branch: true,
						}
					}
					ContinuationKind::Leaf => {
						let description = cont.value.map_or_else(String::new, |entry| {
							if !entry.short_desc.is_empty() {
								entry.short_desc.to_string()
							} else if !entry.description.is_empty() {
								entry.description.to_string()
							} else {
								entry.action_name.to_string()
							}
						});
						UtilityWhichKeyEntry {
							key,
							description,
							is_branch: false,
						}
					}
				}
			})
			.collect();

		Some(UtilityWhichKeyPlan {
			root,
			root_description,
			entries,
		})
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
}
