use std::time::Duration;

use tome_core::ext::notifications::{
	Anchor, Animation, Level, Notification, SizeConstraint, Timing,
};
use tome_core::ext::{
	CommandSource, CompletionContext, CompletionItem, CompletionKind, CompletionSource,
};

use crate::editor::Editor;
use crate::editor::types::{Message, MessageKind};

impl Editor {
	pub fn request_redraw(&mut self) {
		self.needs_redraw = true;
	}

	pub fn show_message(&mut self, text: impl Into<String>) {
		let text = text.into();
		self.message = Some(Message {
			text: text.clone(),
			kind: MessageKind::Info,
		});

		let style = ratatui::style::Style::default()
			.bg(self.theme.colors.popup.bg)
			.fg(self.theme.colors.popup.fg);

		if let Ok(notif) = Notification::builder(text)
			.level(Level::Info)
			.animation(Animation::Fade)
			.anchor(Anchor::BottomRight)
			.timing(
				Timing::Fixed(Duration::from_millis(200)),
				Timing::Fixed(Duration::from_secs(3)),
				Timing::Fixed(Duration::from_millis(200)),
			)
			.max_size(SizeConstraint::Absolute(40), SizeConstraint::Absolute(5))
			.style(style)
			.build()
		{
			let _ = self.notifications.add(notif);
		}
	}

	pub fn show_error(&mut self, text: impl Into<String>) {
		let text = text.into();
		self.message = Some(Message {
			text: text.clone(),
			kind: MessageKind::Error,
		});

		let style = ratatui::style::Style::default()
			.bg(self.theme.colors.popup.bg)
			.fg(self.theme.colors.status.error_fg);

		if let Ok(notif) = Notification::builder(text)
			.level(Level::Error)
			.animation(Animation::Fade)
			.anchor(Anchor::BottomRight)
			.timing(
				Timing::Fixed(Duration::from_millis(200)),
				Timing::Fixed(Duration::from_secs(5)),
				Timing::Fixed(Duration::from_millis(200)),
			)
			.max_size(SizeConstraint::Absolute(40), SizeConstraint::Absolute(5))
			.style(style)
			.build()
		{
			let _ = self.notifications.add(notif);
		}
	}

	pub fn notify(&mut self, type_name: &str, text: impl Into<String>) {
		use tome_core::ext::notifications::find_notification_type;
		let text = text.into();
		let type_def = find_notification_type(type_name);

		let level = type_def.map(|t| t.level).unwrap_or(Level::Info);
		let auto_dismiss = type_def.and_then(|t| t.auto_dismiss).unwrap_or_default();

		let mut builder = Notification::builder(text)
			.level(level)
			.auto_dismiss(auto_dismiss)
			.animation(Animation::Fade)
			.anchor(Anchor::BottomRight)
			.timing(
				Timing::Fixed(Duration::from_millis(200)),
				Timing::Auto, // uses auto_dismiss for dwell
				Timing::Fixed(Duration::from_millis(200)),
			)
			.max_size(SizeConstraint::Absolute(40), SizeConstraint::Absolute(5));

		if let Some(t) = type_def
			&& let Some(style) = t.style
		{
			builder = builder.style(style);
		}

		if let Ok(notif) = builder.build() {
			let _ = self.notifications.add(notif);
		}
	}

	pub fn update_completions(&mut self) {
		if let Some((prompt, input)) = self.input.command_line() {
			let ctx = CompletionContext {
				input: input.to_string(),
				cursor: input.len(),
				prompt,
			};

			let mut items = CommandSource.complete(&ctx);

			for full_name in self.plugins.commands.keys() {
				if full_name.starts_with(input) {
					items.push(CompletionItem {
						label: full_name.clone(),
						insert_text: full_name.clone(),
						detail: None,
						filter_text: None,
						kind: CompletionKind::Plugin,
					});
				}
			}

			// Deduplicate by label (native commands might be shadowed by plugin names if they were to overlap, but they don't yet)
			items.sort_by(|a, b| a.label.cmp(&b.label));
			items.dedup_by(|a, b| a.label == b.label);

			self.completions.items = items;
			self.completions.active = !self.completions.items.is_empty();
			// Keep selection if still valid, otherwise reset
			if let Some(idx) = self.completions.selected_idx
				&& idx >= self.completions.items.len()
			{
				self.completions.selected_idx = None;
			}
		} else {
			self.completions.active = false;
			self.completions.items.clear();
			self.completions.selected_idx = None;
		}
	}
}
