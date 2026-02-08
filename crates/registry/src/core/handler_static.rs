/// Generic static registration payload for inventory-backed handlers.
pub struct HandlerStatic<F> {
	/// Handler name (must match KDL metadata name).
	pub name: &'static str,
	/// Crate that defined this handler.
	pub crate_name: &'static str,
	/// Handler function pointer.
	pub handler: F,
}
