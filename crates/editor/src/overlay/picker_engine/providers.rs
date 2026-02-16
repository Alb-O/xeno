//! Provider trait for picker candidate generation.

use crate::completion::CompletionItem;

/// Capability contract for picker candidate providers.
pub trait PickerProvider {
	/// Returns candidates for the active picker query.
	fn candidates(&mut self, query: &str) -> Vec<CompletionItem>;
}

/// Closure-backed provider adapter.
pub struct FnPickerProvider<F> {
	f: F,
}

impl<F> FnPickerProvider<F> {
	pub fn new(f: F) -> Self {
		Self { f }
	}
}

impl<F> PickerProvider for FnPickerProvider<F>
where
	F: FnMut(&str) -> Vec<CompletionItem>,
{
	fn candidates(&mut self, query: &str) -> Vec<CompletionItem> {
		(self.f)(query)
	}
}
