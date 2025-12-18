use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use parking_lot::Mutex;
use tokio::sync::oneshot;
use tome_cabi_types::{TomeHostV2, TomePanelId, TomePermissionRequestId};

use crate::events::SendEvent;

#[derive(Clone, Copy)]
pub struct SendPtr<T>(pub *const T);
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

#[derive(Clone)]
pub struct SharedState {
	pub events: Arc<Mutex<VecDeque<SendEvent>>>,
	pub panel_id: Arc<Mutex<Option<TomePanelId>>>,
	pub last_assistant_text: Arc<Mutex<String>>,
	pub pending_permissions: Arc<Mutex<HashMap<TomePermissionRequestId, oneshot::Sender<String>>>>,
	pub next_permission_id: Arc<AtomicU64>,
	pub workspace_root: Arc<Mutex<Option<PathBuf>>>,
}

impl SharedState {
	pub fn new() -> Self {
		Self {
			events: Arc::new(Mutex::new(VecDeque::new())),
			panel_id: Arc::new(Mutex::new(None)),
			last_assistant_text: Arc::new(Mutex::new(String::new())),
			pending_permissions: Arc::new(Mutex::new(HashMap::new())),
			next_permission_id: Arc::new(AtomicU64::new(1)),
			workspace_root: Arc::new(Mutex::new(None)),
		}
	}
}

pub struct HostHandle {
	pub host: SendPtr<TomeHostV2>,
}

impl HostHandle {
	pub fn new(host: *const TomeHostV2) -> Self {
		Self {
			host: SendPtr(host),
		}
	}

	#[allow(dead_code)]
	pub unsafe fn get(&self) -> &TomeHostV2 {
		unsafe { &*self.host.0 }
	}
}
