use super::scheduling::ViewportLane;
use super::*;

impl SyntaxManager {
	/// Drains all completed background tasks and queues results for installation.
	pub fn drain_finished_inflight(&mut self) -> bool {
		let mut any_drained = false;
		let results = self.collector.drain_finished();

		for res in results {
			if let Some(entry) = self.entries.get_mut(&res.doc_id) {
				// Clear the appropriate lane if it matches the finished task
				match res.viewport_lane {
					Some(ViewportLane::Urgent) if entry.sched.lanes.viewport_urgent.active == Some(res.id) => {
						entry.sched.lanes.viewport_urgent.active = None;
					}
					Some(ViewportLane::Enrich) if entry.sched.lanes.viewport_enrich.active == Some(res.id) => {
						entry.sched.lanes.viewport_enrich.active = None;
					}
					_ => {}
				}
				if entry.sched.lanes.bg.active == Some(res.id) {
					entry.sched.lanes.bg.active = None;
				}

				// Epoch check: discard stale results
				if entry.sched.epoch != res.epoch {
					continue;
				}

				entry.sched.completed.push_back(CompletedSyntaxTask {
					doc_version: res.doc_version,
					lang_id: res.lang_id,
					opts: res.opts_key,
					result: res.result,
					class: res.class,
					elapsed: res.elapsed,
					viewport_key: res.viewport_key,
					viewport_lane: res.viewport_lane,
				});
				any_drained = true;
			}
		}
		any_drained
	}
}
