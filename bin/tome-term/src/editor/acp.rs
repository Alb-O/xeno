//! ACP-related methods for the Editor.

use std::path::PathBuf;

use crate::acp::{AcpEvent, ChatItem, ChatPanelState, ChatRole};
use crate::editor::Editor;
use crate::ui::panels::chat::{AcpChatPanel, chat_panel_ui_id};

/// Panel ID for the ACP chat panel.
pub const ACP_PANEL_ID: u64 = u64::MAX - 1; // Reserve a special ID for ACP

impl Editor {
	/// Start the ACP agent.
	pub fn acp_start(&mut self, cwd: PathBuf) -> Result<(), tome_core::ext::CommandError> {
		self.acp.start(cwd);
		Ok(())
	}

	/// Stop the ACP agent.
	pub fn acp_stop(&mut self) -> Result<(), tome_core::ext::CommandError> {
		self.acp.stop();
		Ok(())
	}

	/// Toggle the ACP chat panel.
	pub fn acp_toggle(&mut self) -> Result<(), tome_core::ext::CommandError> {
		let panel_id = self.acp.panel_id().unwrap_or(ACP_PANEL_ID);

		let mut panels = self.acp.state.panels.lock();
		// Create the panel state if it doesn't exist
		if let std::collections::hash_map::Entry::Vacant(e) = panels.entry(panel_id) {
			e.insert(ChatPanelState::new("ACP Agent".to_string()));

			self.ui.register_panel(Box::new(AcpChatPanel::new(
				panel_id,
				"ACP Agent".to_string(),
			)));

			self.acp.set_panel_id(Some(panel_id));
		}

		let ui_id = chat_panel_ui_id(panel_id);
		self.ui.toggle_panel(&ui_id);
		self.needs_redraw = true;
		Ok(())
	}

	/// Insert the last ACP assistant response.
	pub fn acp_insert_last(&mut self) -> Result<(), tome_core::ext::CommandError> {
		let text = self.acp.last_assistant_text();
		if text.is_empty() {
			return Err(tome_core::ext::CommandError::Failed(
				"No assistant response available".to_string(),
			));
		}
		self.insert_text(&text);
		Ok(())
	}

	/// Cancel the current ACP request.
	pub fn acp_cancel(&mut self) -> Result<(), tome_core::ext::CommandError> {
		self.acp.cancel();
		Ok(())
	}

	/// Submit the ACP panel input.
	pub fn submit_acp_panel(&mut self) {
		let panel_id = match self.acp.panel_id() {
			Some(id) => id,
			None => return,
		};

		let mut panels = self.acp.state.panels.lock();
		if let Some(panel) = panels.get_mut(&panel_id) {
			let text = panel.input.to_string();
			if text.trim().is_empty() {
				return;
			}

			// Add user message to transcript
			panel.transcript.push(ChatItem {
				role: ChatRole::User,
				text: text.clone(),
			});
			panel.input = "".into();
			panel.input_cursor = 0;

			self.acp.prompt(text);
		}
	}

	/// Poll and process ACP events.
	pub fn poll_acp_events(&mut self) {
		let events = self.acp.drain_events();
		for event in events {
			self.handle_acp_event(event);
		}
	}

	fn handle_acp_event(&mut self, event: AcpEvent) {
		match event {
			AcpEvent::PanelAppend { role, text } => {
				let panel_id = match self.acp.panel_id() {
					Some(id) => id,
					None => return,
				};

				let mut panels = self.acp.state.panels.lock();
				if let Some(panel) = panels.get_mut(&panel_id) {
					panel.transcript.push(ChatItem { role, text });
					self.needs_redraw = true;
				}
			}
			AcpEvent::ShowMessage(msg) => {
				self.show_message(msg);
			}
			AcpEvent::RequestPermission { id, prompt, .. } => {
				// For now, show a message and auto-allow
				// TODO: implement proper permission UI
				self.show_message(format!("ACP permission request: {}", prompt));
				self.acp.on_permission_decision(id, "allow".to_string());
			}
		}
	}
}
