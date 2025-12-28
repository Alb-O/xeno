//! Notification data model and type registration.
//!
//! This module provides the abstract `Notification` data model and
//! convenience traits for creating notifications. The actual rendering
//! is handled by `evildoer_api`.
//!
//! # Architecture
//!
//! - **Data model** (`notification/`): Abstract `Notification` struct using
//!   types from `evildoer_base` and `evildoer_manifest`. No UI dependencies.
//!
//! - **Type registration** (`defaults.rs`): Registers built-in notification
//!   types (info, warn, error, etc.) and creates convenience extension traits.
//!
//! - **Type re-exports** (`types.rs`): Re-exports types from `evildoer_manifest`
//!   for convenience.
//!
//! # Usage
//!
//! ```ignore
//! use evildoer_stdlib::notifications::{Notification, NotificationBuilder};
//!
//! let notif = NotificationBuilder::from_registry("info", "Hello!")
//!     .title("Greeting")
//!     .build()?;
//! ```

mod defaults;
mod notification;
mod types;

pub use defaults::{
	NotifyDEBUGExt, NotifyERRORExt, NotifyINFOExt, NotifySUCCESSExt, NotifyWARNExt,
};
pub use notification::{MAX_CONTENT_CHARS, Notification, NotificationBuilder};
pub use evildoer_manifest::notifications::{
	NOTIFICATION_TYPES, NotificationTypeDef, find_notification_type,
};
pub use types::*;
