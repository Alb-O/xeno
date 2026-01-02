//! Registry-first organization for Evildoer editor extensions.
//!
//! This crate aggregates all registry sub-crates. Depend on this crate to get
//! access to all registries, rather than depending on individual registry crates.
//!
//! # Sub-crates
//!
//! - [`menus`] - Menu bar groups and items
//! - [`motions`] - Cursor movement primitives
//! - [`options`] - Configuration options
//!
//! # Adding a New Registry
//!
//! 1. Create `crates/registry/{name}/` with Cargo.toml and src/
//! 2. Add to root `Cargo.toml` members and workspace.dependencies
//! 3. Add dependency and re-export here

// Re-export commonly used items at the crate root for convenience
pub use menus::{menu_group, menu_item, MenuGroupDef, MenuItemDef, MENU_GROUPS, MENU_ITEMS};
// Re-export shared types (these are duplicated across registries, pick one source)
pub use motions::{flags, Capability, RegistrySource};
pub use motions::{motion, movement, MotionDef, MotionHandler, MOTIONS};
pub use options::{option, OptionDef, OptionScope, OptionType, OptionValue, OPTIONS};
pub use {
	evildoer_registry_menus as menus, evildoer_registry_motions as motions,
	evildoer_registry_options as options,
};
