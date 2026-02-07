use xeno_registry::CommandQueueAccess;

use crate::capabilities::provider::EditorCaps;

impl CommandQueueAccess for EditorCaps<'_> {
	fn queue_command(&mut self, name: &'static str, args: Vec<String>) {
		self.ed.state.effects.queue_command(name, args);
	}
}
