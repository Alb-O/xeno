//! KDL-based registry definition pipeline.
//!
//! Handles loading precompiled metadata from KDL blobs and linking
//! it with Rust handler functions registered via domain-specific handler macros.

#[cfg(any(
	feature = "actions",
	feature = "commands",
	feature = "motions",
	feature = "textobj",
	feature = "options",
	feature = "gutter",
	feature = "statusline",
	feature = "hooks"
))]
pub mod link;
pub mod loader;
pub mod types;
