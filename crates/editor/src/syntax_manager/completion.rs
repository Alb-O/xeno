use super::*;

impl SyntaxManager {
	/// Drains all completed background tasks and installs results if valid.
	pub fn drain_finished_inflight(&mut self) -> bool {
		let mut any_drained = false;
		let results = self.collector.drain_finished();

		for res in results {
			if let Some(entry) = self.entries.get_mut(&res.doc_id) {
				// Clear active_task if it matches the one that just finished, regardless of epoch
				if entry.sched.active_task == Some(res.id) {
					entry.sched.active_task = None;
					entry.sched.active_task_class = None;
					entry.sched.active_task_detached = false;
				}

				// Epoch check: discard stale results
				if entry.sched.epoch != res.epoch {
					continue;
				}

				entry.sched.completed = Some(CompletedSyntaxTask {
					doc_version: res.doc_version,
					lang_id: res.lang_id,
					opts: res.opts_key,
					result: res.result,
					class: res.class,
					injections: res.injections,
					elapsed: res.elapsed,
				});
				any_drained = true;
			}
		}
		any_drained
	}
}
