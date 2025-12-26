use crate::editor::Editor;
use crate::editor::types::{Message, MessageKind};

impl Editor {
	#[allow(
		dead_code,
		reason = "Method currently unused but intended for future extension use cases"
	)]
	pub fn request_redraw(&mut self) {
		self.needs_redraw = true;
	}

	pub fn notify(&mut self, type_name: &str, text: impl Into<String>) {
		use tome_stdlib::notifications::{
			Level as NotifLevel, NotificationBuilder, find_notification_type,
		};
		let text = text.into();

		// Update legacy message field for CLI and status line
		let type_def = find_notification_type(type_name);
		let kind = match type_def.map(|t| t.level).unwrap_or(NotifLevel::Info) {
			NotifLevel::Error => MessageKind::Error,
			NotifLevel::Warn => MessageKind::Warning,
			_ => MessageKind::Info,
		};
		self.message = Some(Message {
			text: text.clone(),
			kind,
		});

		let builder = NotificationBuilder::from_registry(type_name, text);

		// Resolve semantic style from theme (with inheritance)
		let semantic = type_def
			.map(|t| t.semantic)
			.unwrap_or(tome_manifest::SEMANTIC_INFO);
		let style = self.theme.colors.notification_style(semantic);

		if let Ok(notif) = builder.style(style).build() {
			let _ = self.notifications.add(notif);
		}
	}
}
