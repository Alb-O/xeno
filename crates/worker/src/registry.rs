use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::TaskClass;

/// Snapshot for one registered worker actor/service.
#[derive(Debug, Clone)]
pub struct WorkerRecord {
	pub name: String,
	pub class: TaskClass,
	pub generation: u64,
	pub restarts: usize,
	pub pending: usize,
	pub last_exit: Option<String>,
}

/// In-memory worker registry for status snapshots.
#[derive(Debug, Default, Clone)]
pub struct WorkerRegistry {
	inner: Arc<RwLock<HashMap<String, WorkerRecord>>>,
}

impl WorkerRegistry {
	/// Creates an empty registry.
	pub fn new() -> Self {
		Self::default()
	}

	/// Upserts one record.
	pub fn upsert(&self, record: WorkerRecord) {
		if let Ok(mut guard) = self.inner.write() {
			guard.insert(record.name.clone(), record);
		}
	}

	/// Removes one record.
	pub fn remove(&self, name: &str) {
		if let Ok(mut guard) = self.inner.write() {
			guard.remove(name);
		}
	}

	/// Returns snapshots sorted by name.
	pub fn snapshots(&self) -> Vec<WorkerRecord> {
		let Ok(guard) = self.inner.read() else {
			return Vec::new();
		};
		let mut records: Vec<_> = guard.values().cloned().collect();
		records.sort_by(|a, b| a.name.cmp(&b.name));
		records
	}
}
