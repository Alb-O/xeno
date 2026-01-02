//! Notification extension traits and type re-exports.
//!
//! This module provides convenience traits for emitting notifications
//! and re-exports types from `evildoer_registry::notifications`.
//!
//! The actual notification rendering uses `evildoer_tui::widgets::notifications::Toast`.

mod extensions;
mod types;

pub use evildoer_registry::notifications::{
	find_notification_type, NotificationTypeDef, NOTIFICATION_TYPES,
};
pub use extensions::{
	NotifyDEBUGExt, NotifyERRORExt, NotifyINFOExt, NotifySUCCESSExt, NotifyWARNExt,
};
pub use types::*;
