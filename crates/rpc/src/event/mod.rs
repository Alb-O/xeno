//! Dynamic runtime event type for user-defined loopback events.

use std::any::{Any, TypeId, type_name};
use std::fmt;

/// A dynamic runtime event.
///
/// This is a wrapper of `Box<dyn Any + Send>`, but saves the underlying type name for better
/// `Debug` impl.
pub struct AnyEvent {
	/// The boxed event value.
	inner: Box<dyn Any + Send>,
	/// The original type name for debugging.
	type_name: &'static str,
}

impl fmt::Debug for AnyEvent {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("AnyEvent").field("type_name", &self.type_name).finish_non_exhaustive()
	}
}

impl AnyEvent {
	/// Creates a new event wrapping the given value.
	#[must_use]
	pub fn new<T: Send + 'static>(v: T) -> Self {
		AnyEvent {
			inner: Box::new(v),
			type_name: type_name::<T>(),
		}
	}

	/// Returns the `TypeId` of the inner value.
	#[must_use]
	pub fn inner_type_id(&self) -> TypeId {
		// Call `type_id` on the inner `dyn Any`, not `Box<_> as Any` or `&Box<_> as Any`.
		Any::type_id(&*self.inner)
	}

	/// Get the underlying type name for debugging purpose.
	///
	/// The result string is only meant for debugging. It is not stable and cannot be trusted.
	#[must_use]
	pub fn type_name(&self) -> &'static str {
		self.type_name
	}

	/// Returns `true` if the inner type is the same as `T`.
	#[must_use]
	pub fn is<T: Send + 'static>(&self) -> bool {
		self.inner.is::<T>()
	}

	/// Returns some reference to the inner value if it is of type `T`, or `None` if it isn't.
	#[must_use]
	pub fn downcast_ref<T: Send + 'static>(&self) -> Option<&T> {
		self.inner.downcast_ref::<T>()
	}

	/// Returns some mutable reference to the inner value if it is of type `T`, or `None` if it
	/// isn't.
	#[must_use]
	pub fn downcast_mut<T: Send + 'static>(&mut self) -> Option<&mut T> {
		self.inner.downcast_mut::<T>()
	}

	/// Attempt to downcast it to a concrete type.
	///
	/// # Errors
	///
	/// Returns `self` if the type mismatches.
	pub fn downcast<T: Send + 'static>(self) -> Result<T, Self> {
		match self.inner.downcast::<T>() {
			Ok(v) => Ok(*v),
			Err(inner) => Err(Self {
				inner,
				type_name: self.type_name,
			}),
		}
	}
}

#[cfg(test)]
mod tests;
