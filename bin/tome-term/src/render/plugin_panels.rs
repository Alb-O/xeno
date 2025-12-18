use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use tome_cabi_types::TomeChatRole;

use crate::editor::Editor;

impl Editor {
	pub fn render_plugin_panels(&mut self, frame: &mut ratatui::Frame) {
		if self.plugins.plugins_open {
			self.render_plugins_panel(frame);
		}

		for panel in self.plugins.panels.values() {
			if !panel.open {
				continue;
			}

			let area = frame.area();
			let popup_height = (area.height * 40 / 100).max(10);
			let popup_area = Rect {
				x: area.x,
				y: area.height.saturating_sub(popup_height + 2), // above status/message
				width: area.width,
				height: popup_height,
			};

			frame.render_widget(Clear, popup_area);
			let block = Block::default()
				.borders(Borders::ALL)
				.title(format!(" {} ", panel.title))
				.style(
					Style::default()
						.bg(self.theme.colors.popup.bg)
						.fg(self.theme.colors.popup.fg),
				);

			let inner = block.inner(popup_area);
			frame.render_widget(block, popup_area);

			let chunks = Layout::default()
				.direction(Direction::Vertical)
				.constraints([
					Constraint::Min(1),    // Transcript
					Constraint::Length(3), // Input
				])
				.split(inner);

			// Render transcript
			let mut lines = Vec::new();
			for item in &panel.transcript {
				let (prefix, style) = match item.role {
					TomeChatRole::User => ("User: ", Style::default().add_modifier(Modifier::BOLD)),
					TomeChatRole::Assistant => (
						"Assistant: ",
						Style::default()
							.add_modifier(Modifier::BOLD)
							.fg(Color::Cyan),
					),
					TomeChatRole::System => (
						"System: ",
						Style::default()
							.add_modifier(Modifier::BOLD)
							.fg(Color::Yellow),
					),
					TomeChatRole::Thought => (
						"(thought) ",
						Style::default()
							.add_modifier(Modifier::ITALIC)
							.fg(Color::DarkGray),
					),
				};

				lines.push(ratatui::text::Line::from(vec![
					ratatui::text::Span::styled(prefix, style),
					ratatui::text::Span::raw(&item.text),
				]));
			}

			let transcript = Paragraph::new(lines);
			frame.render_widget(transcript, chunks[0]);

			// Render input
			let input_text = panel.input.to_string();
			let input = Paragraph::new(input_text)
				.block(Block::default().borders(Borders::TOP).title(" Input "));
			frame.render_widget(input, chunks[1]);

			if panel.focused {
				frame.set_cursor_position(ratatui::layout::Position {
					x: chunks[1].x + (panel.input_cursor % chunks[1].width as usize) as u16,
					y: chunks[1].y + 1 + (panel.input_cursor / chunks[1].width as usize) as u16,
				});
			}
		}
	}

	pub fn render_plugins_panel(&mut self, frame: &mut ratatui::Frame) {
		let area = frame.area();
		let popup_height = (area.height * 60 / 100).max(15);
		let popup_width = (area.width * 80 / 100).max(40);
		let popup_area = Rect {
			x: area.x + (area.width - popup_width) / 2,
			y: (area.height.saturating_sub(popup_height + 2)) / 2,
			width: popup_width,
			height: popup_height,
		};

		frame.render_widget(Clear, popup_area);
		let block = Block::default()
			.borders(Borders::ALL)
			.title(" Plugins ")
			.style(
				Style::default()
					.bg(self.theme.colors.popup.bg)
					.fg(self.theme.colors.popup.fg),
			);

		let inner = block.inner(popup_area);
		frame.render_widget(block, popup_area);

		let mut lines = Vec::new();
		let mut sorted_ids: Vec<_> = self.plugins.entries.keys().collect();
		sorted_ids.sort();

		for (i, id) in sorted_ids.iter().enumerate() {
			let entry = &self.plugins.entries[*id];
			let is_selected = i == self.plugins.plugins_selected_idx;
			let style = if is_selected {
				Style::default()
					.bg(Color::Indexed(240)) // Darker gray for selection
					.add_modifier(Modifier::BOLD)
			} else {
				Style::default()
			};

			let status_str = match &entry.status {
				crate::plugin::manager::PluginStatus::Installed => "Installed",
				crate::plugin::manager::PluginStatus::Loaded => "Loaded",
				crate::plugin::manager::PluginStatus::Failed(_) => "Failed",
				crate::plugin::manager::PluginStatus::Disabled => "Disabled",
			};

			let status_color = match &entry.status {
				crate::plugin::manager::PluginStatus::Loaded => Color::Green,
				crate::plugin::manager::PluginStatus::Failed(_) => Color::Red,
				crate::plugin::manager::PluginStatus::Disabled => Color::DarkGray,
				_ => Color::Gray,
			};

			lines.push(ratatui::text::Line::from(vec![
				ratatui::text::Span::styled(format!("{:<15} ", id), style),
				ratatui::text::Span::styled(format!("v{:<8} ", entry.manifest.version), style),
				ratatui::text::Span::styled(format!("ABI:{} ", entry.manifest.abi), style),
				ratatui::text::Span::styled(status_str, Style::default().fg(status_color)),
			]));

			if is_selected {
				if let Some(desc) = &entry.manifest.description {
					lines.push(ratatui::text::Line::from(vec![
						ratatui::text::Span::styled(
							format!("  {}", desc),
							Style::default()
								.fg(Color::Gray)
								.add_modifier(Modifier::ITALIC),
						),
					]));
				}
				if let crate::plugin::manager::PluginStatus::Failed(e) = &entry.status {
					lines.push(ratatui::text::Line::from(vec![
						ratatui::text::Span::styled(
							format!("  Error: {}", e),
							Style::default()
								.fg(Color::Red)
								.add_modifier(Modifier::ITALIC),
						),
					]));
				}
			}
		}

		if lines.is_empty() {
			lines.push(ratatui::text::Line::from("No plugins found."));
		}

		// Surface pending permissions
		if !self.pending_permissions.is_empty() {
			lines.push(ratatui::text::Line::from(""));
			lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
				" Pending Permissions: ",
				Style::default()
					.add_modifier(Modifier::BOLD)
					.fg(Color::Yellow),
			)));
			for perm in &self.pending_permissions {
				lines.push(ratatui::text::Line::from(vec![
					ratatui::text::Span::raw(format!(
						"  [#{}] {}: ",
						perm.request_id, perm.plugin_id
					)),
					ratatui::text::Span::raw(&perm._prompt),
				]));
				let mut options = Vec::new();
				for (id, label) in &perm._options {
					options.push(format!("{} ({})", id, label));
				}
				lines.push(ratatui::text::Line::from(vec![
					ratatui::text::Span::styled(
						format!("    Options: {}", options.join(", ")),
						Style::default().fg(Color::Cyan),
					),
				]));
			}
		}

		let list = Paragraph::new(lines);
		frame.render_widget(list, inner);
	}
}
