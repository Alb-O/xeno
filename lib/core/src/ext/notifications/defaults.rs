use std::time::Duration;

use crate::ext::notifications::{AutoDismiss, Level};
use crate::notification_type;

notification_type!(
	INFO,
	"info",
	Level::Info,
	None,
	None,
	Some(AutoDismiss::After(Duration::from_secs(4)))
);

notification_type!(
	WARN,
	"warn",
	Level::Warn,
	None,
	None,
	Some(AutoDismiss::After(Duration::from_secs(6)))
);

notification_type!(
	ERROR,
	"error",
	Level::Error,
	None,
	None,
	Some(AutoDismiss::Never)
);
