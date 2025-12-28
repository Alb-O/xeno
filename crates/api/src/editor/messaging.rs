use evildoer_manifest::notifications as manifest;
use evildoer_stdlib::notifications::find_notification_type;
use evildoer_tui::style::Style;
use evildoer_tui::widgets::icon::presets as icon_presets;
use evildoer_tui::widgets::notifications::{self as notif, Toast, ToastIcon};

use crate::editor::Editor;

/// Returns the appropriate icon glyph for a semantic notification type.
fn icon_for_semantic(semantic: &str) -> Option<&'static str> {
	match semantic {
		evildoer_manifest::SEMANTIC_INFO => Some(icon_presets::INFO),
		evildoer_manifest::SEMANTIC_WARNING => Some(icon_presets::WARNING),
		evildoer_manifest::SEMANTIC_ERROR => Some(icon_presets::ERROR),
		evildoer_manifest::SEMANTIC_SUCCESS => Some(icon_presets::SUCCESS),
		evildoer_manifest::SEMANTIC_DIM => Some(icon_presets::DEBUG),
		_ => None,
	}
}

impl Editor {
	pub fn notify(&mut self, type_name: &str, text: impl Into<String>) {
		let text = text.into();
		let type_def = find_notification_type(type_name);

		let semantic = type_def
			.map(|t| t.semantic)
			.unwrap_or(evildoer_manifest::SEMANTIC_INFO);
		let notif_style: Style = self.theme.colors.notification_style(semantic).into();
		let accent = notif_style.fg.unwrap_or_default();

		let mut toast = Toast::new(text)
			.style(notif_style)
			.border_style(Style::default().fg(accent));

		if let Some(glyph) = icon_for_semantic(semantic) {
			toast = toast.icon(ToastIcon::new(glyph).style(Style::default().fg(accent)));
		}

		if let Some(def) = type_def {
			toast = toast
				.animation(match def.animation {
					manifest::Animation::Slide => notif::Animation::Slide,
					manifest::Animation::ExpandCollapse => notif::Animation::ExpandCollapse,
					manifest::Animation::Fade => notif::Animation::Fade,
				})
				.auto_dismiss(match def.auto_dismiss {
					manifest::AutoDismiss::Never => notif::AutoDismiss::Never,
					manifest::AutoDismiss::After(d) => notif::AutoDismiss::After(d),
				});
		}

		self.notifications.push(toast);
	}
}
