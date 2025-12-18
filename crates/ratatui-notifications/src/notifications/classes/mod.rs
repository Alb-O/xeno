pub(crate) mod cls_notification;
pub(crate) mod cls_notification_state;

// Public exports
pub use cls_notification::{Notification, NotificationBuilder};
// Internal exports
pub(crate) use cls_notification_state::{ManagerDefaults, NotificationState};
