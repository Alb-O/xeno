use std::path::PathBuf;
use std::time::Duration;

mod app;
mod event_bridge;
mod platform;
mod snapshot;

pub use self::app::run;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Debug, Clone, Default)]
pub struct StartupOptions {
	pub path: Option<PathBuf>,
	pub theme: Option<String>,
}

impl StartupOptions {
	pub fn from_env() -> Self {
		let mut path: Option<PathBuf> = None;
		let mut theme: Option<String> = None;
		let mut args = std::env::args().skip(1);

		while let Some(arg) = args.next() {
			if arg == "--theme" {
				theme = args.next();
				continue;
			}
			if path.is_none() {
				path = Some(PathBuf::from(arg));
			}
		}

		Self { path, theme }
	}
}

pub(crate) use self::event_bridge::{CellMetrics, EventBridgeState, map_event};
pub(crate) use self::platform::configure_linux_backend;
pub(crate) use self::snapshot::{HeaderSnapshot, Snapshot, SurfaceSnapshot, build_snapshot};
