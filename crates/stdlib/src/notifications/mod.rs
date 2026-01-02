//! Notification data model and type registration.
//!
//! This module provides the abstract `Notification` data model and
//! convenience traits for creating notifications. The actual rendering
//! is handled by `evildoer_api`.
//!
//! # Architecture
//!
//! - **Data model** (`notification/`): Abstract `Notification` struct using
//!   types from `evildoer_base` and `evildoer_registry`. No UI dependencies.
//!
//! - **Type registration** (`evildoer-registry-notifications`): Registers built-in
//!   notification types (info, warn, error, etc.).
//!
//! - **Extensions** (`extensions.rs`): Convenience helper traits for notifications.
//!
//! - **Type re-exports** (`types.rs`): Re-exports types from `evildoer_registry`
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

mod extensions;
mod notification;
mod types;

pub use evildoer_registry::notifications::{
	find_notification_type, NotificationTypeDef, NOTIFICATION_TYPES,
};
pub use extensions::{
	NotifyDEBUGExt, NotifyERRORExt, NotifyINFOExt, NotifySUCCESSExt, NotifyWARNExt,
};
pub use notification::{Notification, NotificationBuilder, MAX_CONTENT_CHARS};
pub use types::*;
