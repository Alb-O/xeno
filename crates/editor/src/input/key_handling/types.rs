/// Result of attempting to dispatch an action from a key result.
pub(crate) enum ActionDispatch {
	/// Action was executed; bool indicates quit request.
	Executed(bool),
	/// Key result was not an action.
	NotAction,
}
