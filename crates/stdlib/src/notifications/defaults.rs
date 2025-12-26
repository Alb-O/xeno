use std::time::Duration;

use tome_macro::register_notification;
use tome_manifest::{
	SEMANTIC_DIM, SEMANTIC_ERROR, SEMANTIC_INFO, SEMANTIC_SUCCESS, SEMANTIC_WARNING,
};

use crate::notifications::{AutoDismiss, Level};

register_notification!(
	INFO,
	"info",
	level: Level::Info,
	semantic: SEMANTIC_INFO,
	dismiss: AutoDismiss::After(Duration::from_secs(4))
);

register_notification!(
	WARN,
	"warn",
	level: Level::Warn,
	semantic: SEMANTIC_WARNING,
	dismiss: AutoDismiss::After(Duration::from_secs(6))
);

register_notification!(
	ERROR,
	"error",
	level: Level::Error,
	semantic: SEMANTIC_ERROR,
	dismiss: AutoDismiss::After(Duration::from_secs(8))
);

register_notification!(
	SUCCESS,
	"success",
	level: Level::Info,
	semantic: SEMANTIC_SUCCESS,
	dismiss: AutoDismiss::After(Duration::from_secs(3))
);

register_notification!(
	DEBUG,
	"debug",
	level: Level::Debug,
	semantic: SEMANTIC_DIM,
	dismiss: AutoDismiss::After(Duration::from_secs(2))
);
