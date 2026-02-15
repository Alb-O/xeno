//! One-shot matching APIs.
//!
//! Provides direct list/indices matching entry points, including parallel list
//! matching helpers.

pub mod bucket;
mod indices;
mod matcher;
mod parallel;

pub use indices::match_indices;
pub use matcher::match_list;
pub use parallel::match_list_parallel;

pub trait Appendable<T> {
	fn append(&mut self, value: T);
}

impl<T> Appendable<T> for Vec<T> {
	fn append(&mut self, value: T) {
		self.push(value);
	}
}
