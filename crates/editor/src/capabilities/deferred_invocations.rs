use xeno_registry::actions::DeferredInvocationAccess;

use crate::capabilities::provider::EditorCaps;

impl DeferredInvocationAccess for EditorCaps<'_> {
	fn defer_command(&mut self, name: String, args: Vec<String>) {
		self.ed.state.effects.defer_command(name, args);
	}
}
