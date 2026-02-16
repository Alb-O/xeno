//! Experimental options used by Xeno's vendored Nu runtime.
//!
//! Xeno only consumes a narrow subset of Nushell's experimental toggles, so this
//! crate intentionally keeps just the option state machinery and the option
//! definitions that are read by the runtime.

use std::fmt::Debug;
use std::sync::atomic::Ordering;

use crate::options::Version;
use crate::util::AtomicMaybe;

mod options;
mod util;

pub use options::*;

/// The status of an experimental option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
	/// Disabled by default.
	OptIn,
	/// Enabled by default.
	OptOut,
	/// Deprecated as an experimental option and now default behavior.
	DeprecatedDefault,
	/// Deprecated and intended to be discarded.
	DeprecatedDiscard,
}

/// Experimental option (feature flag).
pub struct ExperimentalOption {
	value: AtomicMaybe,
	marker: &'static (dyn DynExperimentalOptionMarker + Send + Sync),
}

impl ExperimentalOption {
	/// Construct a new experimental option.
	pub(crate) const fn new(marker: &'static (dyn DynExperimentalOptionMarker + Send + Sync)) -> Self {
		Self {
			value: AtomicMaybe::new(None),
			marker,
		}
	}

	pub fn identifier(&self) -> &'static str {
		self.marker.identifier()
	}

	pub fn description(&self) -> &'static str {
		self.marker.description()
	}

	pub fn status(&self) -> Status {
		self.marker.status()
	}

	pub fn since(&self) -> Version {
		self.marker.since()
	}

	pub fn issue_id(&self) -> u32 {
		self.marker.issue()
	}

	pub fn issue_url(&self) -> String {
		format!("https://github.com/nushell/nushell/issues/{}", self.marker.issue())
	}

	pub fn get(&self) -> bool {
		self.value.load(Ordering::Relaxed).unwrap_or_else(|| match self.marker.status() {
			Status::OptIn => false,
			Status::OptOut => true,
			Status::DeprecatedDiscard => false,
			Status::DeprecatedDefault => false,
		})
	}

	/// Sets the state of an experimental option.
	///
	/// # Safety
	/// These options are expected to be set during initialization and remain
	/// stable afterwards.
	pub unsafe fn set(&self, value: bool) {
		self.value.store(value, Ordering::Relaxed);
	}

	/// Unsets an experimental option.
	///
	/// # Safety
	/// Like [`set`](Self::set), callers must only use this in controlled
	/// initialization contexts.
	pub unsafe fn unset(&self) {
		self.value.store(None, Ordering::Relaxed);
	}
}

impl Debug for ExperimentalOption {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let add_description = f.sign_plus();
		let mut debug_struct = f.debug_struct("ExperimentalOption");
		debug_struct.field("identifier", &self.identifier());
		debug_struct.field("value", &self.get());
		debug_struct.field("stability", &self.status());
		if add_description {
			debug_struct.field("description", &self.description());
		}
		debug_struct.finish()
	}
}

impl PartialEq for ExperimentalOption {
	fn eq(&self, other: &Self) -> bool {
		self.value.as_ptr() == other.value.as_ptr()
	}
}

impl Eq for ExperimentalOption {}

/// Sets all non-deprecated experimental options.
///
/// # Safety
/// Callers should only toggle options during initialization.
pub unsafe fn set_all(value: bool) {
	for option in ALL {
		match option.status() {
			Status::OptIn | Status::OptOut => {
				// SAFETY: The safety contract matches this function.
				unsafe { option.set(value) }
			}
			Status::DeprecatedDefault | Status::DeprecatedDiscard => {}
		}
	}
}

pub(crate) trait DynExperimentalOptionMarker {
	fn identifier(&self) -> &'static str;
	fn description(&self) -> &'static str;
	fn status(&self) -> Status;
	fn since(&self) -> Version;
	fn issue(&self) -> u32;
}

impl<M: options::ExperimentalOptionMarker> DynExperimentalOptionMarker for M {
	fn identifier(&self) -> &'static str {
		M::IDENTIFIER
	}

	fn description(&self) -> &'static str {
		M::DESCRIPTION
	}

	fn status(&self) -> Status {
		M::STATUS
	}

	fn since(&self) -> Version {
		M::SINCE
	}

	fn issue(&self) -> u32 {
		M::ISSUE
	}
}
