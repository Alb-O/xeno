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
					Some(ViewportLane::Urgent) if entry.sched.active_viewport_urgent == Some(res.id) => {
						entry.sched.active_viewport_urgent = None;
						entry.sched.active_viewport_urgent_detached = false;
					}
					Some(ViewportLane::Enrich) if entry.sched.active_viewport_enrich == Some(res.id) => {
						entry.sched.active_viewport_enrich = None;
						entry.sched.active_viewport_enrich_detached = false;
					}
					_ => {}
				}
				if entry.sched.active_bg == Some(res.id) {
					entry.sched.active_bg = None;
					entry.sched.active_bg_detached = false;
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
					injections: res.injections,
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
