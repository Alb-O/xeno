use xeno_registry::actions::DeferredInvocationAccess;

use crate::capabilities::provider::EditorCaps;

impl DeferredInvocationAccess for EditorCaps<'_> {
	fn queue_invocation(&mut self, request: xeno_registry::actions::DeferredInvocationRequest) {
		self.ed.state.runtime.effects.queue_invocation_request(request);
	}
}
