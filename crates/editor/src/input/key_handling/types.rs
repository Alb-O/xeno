/// Result of attempting to dispatch an action from a key result.
pub(crate) enum ActionDispatch {
	/// Action was executed and produced an invocation result.
	Executed(crate::types::InvocationResult),
	/// Key result was not an action.
	NotAction,
}
