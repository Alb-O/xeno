use xeno_registry::actions::NotificationAccess;
use xeno_registry::notifications::Notification;

use crate::capabilities::provider::EditorCaps;

impl NotificationAccess for EditorCaps<'_> {
	fn emit(&mut self, notification: Notification) {
		self.ed.state.runtime.effects.notify(notification);
	}

	fn clear_notifications(&mut self) {
		self.ed.clear_all_notifications();
		self.ed.state.runtime.effects.request_redraw();
	}
}
