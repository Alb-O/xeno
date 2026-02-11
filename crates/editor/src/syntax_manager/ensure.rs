use super::*;

impl SyntaxManager {
	/// Invariant enforcement: Polls or kicks background syntax parsing for a document.
	pub fn ensure_syntax(&mut self, ctx: EnsureSyntaxContext<'_>) -> SyntaxPollOutcome {
		let now = Instant::now();
		let doc_id = ctx.doc_id;

		// Calculate policy and options key
		let bytes = ctx.content.len_bytes();
		let bytes_u32 = bytes as u32;
		let tier = self.policy.tier_for_bytes(bytes);
		let cfg = self.policy.cfg(tier);
		let current_opts_key = OptKey { injections: cfg.injections };
		let viewport = ctx.viewport.as_ref().map(|raw| {
			let start = raw.start.min(bytes_u32);
			let mut end = raw.end.min(bytes_u32);
			if end < start {
				end = start;
			}
			let capped_end = start.saturating_add(cfg.viewport_visible_span_cap);
			end = end.min(capped_end);
			start..end
		});

		let work_disabled = matches!(ctx.hotness, SyntaxHotness::Cold) && !cfg.parse_when_hidden;

		// 1. Initial entry check & change detection
		let mut was_updated = {
			let entry = self.entry_mut(doc_id);
			let mut updated = entry.slot.take_updated();

			if entry.slot.language_id != ctx.language_id {
				entry.sched.invalidate();
				if entry.slot.current.is_some() {
					entry.slot.drop_tree();
					Self::mark_updated(&mut entry.slot);
					updated = true;
				}
				entry.slot.language_id = ctx.language_id;
			}

			if matches!(ctx.hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
				entry.sched.last_visible_at = now;
			}

			if entry.slot.last_opts_key.is_some_and(|k| k != current_opts_key) {
				entry.sched.invalidate();
				entry.slot.dirty = true;
				entry.slot.drop_tree();
				Self::mark_updated(&mut entry.slot);
				updated = true;
			}
			entry.slot.last_opts_key = Some(current_opts_key);

			if entry.sched.active_task.is_some() {
				entry.sched.active_task_detached = work_disabled;
			}

			updated
		};

		// 2. Process completed tasks (from local cache)
		let task_record = {
			let entry = self.entry_mut(doc_id);
			if let Some(done) = entry.sched.completed.take() {
				let lang_id = done.lang_id;
				let class = done.class;
				let injections = done.injections;
				let elapsed = done.elapsed;
				let is_timeout = matches!(done.result, Err(xeno_language::syntax::SyntaxError::Timeout));
				let is_error = done.result.is_err() && !is_timeout;

				match done.result {
					Ok(syntax_tree) => {
						if let Some(current_lang) = ctx.language_id {
							let lang_ok = lang_id == current_lang;
							let opts_ok = if class == TaskClass::Viewport {
								let stage_a_ok = injections == cfg.viewport_injections;
								let stage_b_ok = injections == InjectionPolicy::Eager && cfg.viewport_stage_b_budget.is_some();
								stage_a_ok || stage_b_ok
							} else {
								done.opts == current_opts_key
							};
							let version_match = done.doc_version == ctx.doc_version;

							let retain_ok = Self::retention_allows_install(now, &entry.sched, cfg.retention_hidden, ctx.hotness);

							let allow_install = if class == TaskClass::Viewport {
								let monotonic_ok = entry.slot.tree_doc_version.is_none_or(|v| done.doc_version >= v);
								let not_future = done.doc_version <= ctx.doc_version;
								let requested_ok = done.doc_version >= entry.sched.requested_doc_version;
								monotonic_ok && not_future && requested_ok
							} else {
								Self::should_install_completed_parse(
									done.doc_version,
									entry.slot.tree_doc_version,
									entry.sched.requested_doc_version,
									ctx.doc_version,
									entry.slot.dirty,
								)
							};

							let is_installed = if work_disabled {
								tracing::trace!(?doc_id, "Discarding syntax result because work is disabled (Cold)");
								false
							} else if lang_ok && opts_ok && retain_ok && allow_install {
								let is_viewport = class == TaskClass::Viewport;
								if is_viewport {
									entry.slot.dirty = true;
									entry.sched.force_no_debounce = true;
								}

								if let Some(meta) = &syntax_tree.viewport {
									entry.slot.coverage = Some(meta.base_offset..meta.base_offset.saturating_add(meta.real_len));
								} else {
									entry.slot.coverage = None;
								}

								entry.slot.current = Some(syntax_tree);
								entry.slot.language_id = Some(current_lang);
								entry.slot.tree_doc_version = Some(done.doc_version);
								entry.slot.pending_incremental = None;
								Self::mark_updated(&mut entry.slot);
								was_updated = true;

								if !is_viewport {
									entry.sched.force_no_debounce = false;
									if version_match {
										entry.slot.dirty = false;
										entry.sched.cooldown_until = None;
									} else {
										entry.slot.dirty = true;
									}
								}
								true
							} else {
								tracing::trace!(
									?doc_id,
									?lang_ok,
									?opts_ok,
									?retain_ok,
									?allow_install,
									"Discarding syntax result due to mismatch/retention"
								);
								if lang_ok && opts_ok && version_match && !retain_ok {
									entry.slot.drop_tree();
									entry.slot.dirty = false;
									entry.sched.force_no_debounce = false;
									Self::mark_updated(&mut entry.slot);
									was_updated = true;
								}
								false
							};

							Some((lang_id, tier, class, injections, elapsed, is_timeout, is_error, is_installed))
						} else {
							Some((lang_id, tier, class, injections, elapsed, is_timeout, is_error, false))
						}
					}
					Err(xeno_language::syntax::SyntaxError::Timeout) => {
						let cooldown = if class == TaskClass::Viewport {
							cfg.viewport_cooldown_on_timeout
						} else {
							cfg.cooldown_on_timeout
						};
						entry.sched.cooldown_until = Some(now + cooldown);
						Some((lang_id, tier, class, injections, elapsed, true, false, false))
					}
					Err(e) => {
						tracing::warn!(?doc_id, ?tier, error=%e, "Background syntax parse failed");
						let cooldown = if class == TaskClass::Viewport {
							cfg.viewport_cooldown_on_error
						} else {
							cfg.cooldown_on_error
						};
						entry.sched.cooldown_until = Some(now + cooldown);
						Some((lang_id, tier, class, injections, elapsed, false, true, false))
					}
				}
			} else {
				None
			}
		};

		if let Some((lang_id, tier, class, injections, elapsed, is_timeout, is_error, is_installed)) = task_record {
			self.metrics
				.record_task_result(lang_id, tier, class, injections, elapsed, is_timeout, is_error, is_installed);
			if is_timeout || is_error {
				return SyntaxPollOutcome {
					result: SyntaxPollResult::CoolingDown,
					updated: was_updated,
				};
			}
		}

		{
			let entry = self.entry_mut(doc_id);

			if entry.sched.active_task.is_some() && !entry.sched.active_task_detached {
				if Self::apply_retention(now, &entry.sched, cfg.retention_hidden, ctx.hotness, &mut entry.slot, doc_id) {
					entry.sched.invalidate();
					was_updated = true;
				} else {
					let should_preempt_for_viewport = tier == SyntaxTier::L
						&& ctx.hotness == SyntaxHotness::Visible
						&& matches!(entry.sched.active_task_class, Some(TaskClass::Full | TaskClass::Incremental))
						&& viewport.is_some()
						&& entry.slot.current.as_ref().is_some_and(|s| s.is_partial())
						&& match (&viewport, &entry.slot.coverage) {
							(Some(vp), Some(coverage)) => vp.start < coverage.start || vp.end > coverage.end,
							(Some(_), None) => true,
							_ => false,
						};
					if should_preempt_for_viewport {
						entry.sched.invalidate();
						entry.slot.dirty = true;
					} else {
						return SyntaxPollOutcome {
							result: SyntaxPollResult::Pending,
							updated: was_updated,
						};
					}
				}
			}

			// 4. Language check
			let Some(_lang_id) = ctx.language_id else {
				if entry.slot.current.is_some() {
					entry.slot.drop_tree();
					Self::mark_updated(&mut entry.slot);
					was_updated = true;
				}
				entry.slot.language_id = None;
				entry.slot.dirty = false;
				entry.sched.cooldown_until = None;
				return SyntaxPollOutcome {
					result: SyntaxPollResult::NoLanguage,
					updated: was_updated,
				};
			};

			if Self::apply_retention(now, &entry.sched, cfg.retention_hidden, ctx.hotness, &mut entry.slot, doc_id) {
				if !work_disabled {
					entry.sched.invalidate();
				}
				was_updated = true;
			}

			// 5. Gating
			if work_disabled {
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Disabled,
					updated: was_updated,
				};
			}

			if entry.slot.current.is_some() && !entry.slot.dirty {
				entry.sched.force_no_debounce = false;
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Ready,
					updated: was_updated,
				};
			}

			if entry.slot.current.is_some() && !entry.sched.force_no_debounce && now.duration_since(entry.sched.last_edit_at) < cfg.debounce {
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Pending,
					updated: was_updated,
				};
			}

			if let Some(until) = entry.sched.cooldown_until
				&& now < until
			{
				return SyntaxPollOutcome {
					result: SyntaxPollResult::CoolingDown,
					updated: was_updated,
				};
			}

			// Defensive: never schedule if a task is already active for this document identity
			if let Some(_task_id) = entry.sched.active_task
				&& !entry.sched.active_task_detached
			{
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Pending,
					updated: was_updated,
				};
			}
		}

		// 5.5 Sync bootstrap fast path
		let lang_id = ctx.language_id.unwrap();
		let (do_sync, sync_timeout, pre_epoch) = {
			let entry = self.entry_mut(doc_id);
			let is_bootstrap = entry.slot.current.is_none();
			let is_visible = matches!(ctx.hotness, SyntaxHotness::Visible);

			if is_bootstrap && is_visible && !entry.slot.sync_bootstrap_attempted {
				if let Some(t) = cfg.sync_bootstrap_timeout {
					entry.slot.sync_bootstrap_attempted = true;
					(true, Some(t), entry.sched.epoch)
				} else {
					(false, None, entry.sched.epoch)
				}
			} else {
				(false, None, entry.sched.epoch)
			}
		};

		let sync_result = if do_sync {
			let sync_opts = SyntaxOptions {
				parse_timeout: sync_timeout.unwrap(),
				injections: cfg.injections,
			};
			Some(self.engine.parse(ctx.content.slice(..), lang_id, ctx.loader, sync_opts))
		} else {
			None
		};

		if let Some(res) = sync_result
			&& let Ok(syntax) = res
		{
			let entry = self.entry_mut(doc_id);
			// Re-check invariants after dropping and re-acquiring borrow
			let is_bootstrap = entry.slot.current.is_none();
			let is_visible = matches!(ctx.hotness, SyntaxHotness::Visible);
			if entry.sched.epoch == pre_epoch && is_bootstrap && is_visible && entry.sched.active_task.is_none() {
				entry.slot.current = Some(syntax);
				entry.slot.language_id = Some(lang_id);
				entry.slot.tree_doc_version = Some(ctx.doc_version);
				entry.slot.dirty = false;
				entry.slot.coverage = None;
				entry.slot.viewport_stage_b_attempted = false;
				entry.slot.pending_incremental = None;
				entry.sched.force_no_debounce = false;
				entry.sched.cooldown_until = None;
				Self::mark_updated(&mut entry.slot);
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Ready,
					updated: true,
				};
			}
		}

		// 6. Schedule new task
		// 6a. Viewport-bounded parsing (Stage A and Stage B)
		let (needs_stage_a, needs_stage_b, win_override) = {
			let entry = self.entry_mut(doc_id);
			let mut needs_a = false;
			let mut needs_b = false;
			let mut win = None;

			if tier == SyntaxTier::L
				&& ctx.hotness == SyntaxHotness::Visible
				&& let Some(viewport) = &viewport
			{
				if let Some(current) = &entry.slot.current {
					if current.is_partial() {
						if let Some(coverage) = &entry.slot.coverage {
							if viewport.start < coverage.start || viewport.end > coverage.end {
								needs_a = true;
							} else if current.opts().injections != InjectionPolicy::Eager
								&& !entry.slot.viewport_stage_b_attempted
								&& cfg.viewport_stage_b_budget.is_some()
							{
								needs_b = true;
								win = Some(coverage.clone());
							}
						} else {
							needs_a = true;
						}
					}
				} else {
					needs_a = true;
				}
			}
			(needs_a, needs_b, win)
		};

		let mut really_needs_b = false;
		if needs_stage_b {
			let budget = cfg.viewport_stage_b_budget.unwrap();
			let predicted = self.metrics.predict_duration(lang_id, tier, TaskClass::Viewport, InjectionPolicy::Eager);

			// If no metrics yet, we assume it's within budget (optimistic)
			if predicted.map(|p| p <= budget).unwrap_or(true) {
				really_needs_b = true;
			}
		}

		if (needs_stage_a || really_needs_b) && self.entry_mut(doc_id).sched.active_task.is_none() {
			let mut b_latch_to_set = false;
			let (injections, win_start, win_end) = {
				let viewport = viewport.as_ref().unwrap();

				if needs_stage_a {
					let win_start = viewport.start.saturating_sub(cfg.viewport_lookbehind);
					let mut win_end = viewport.end.saturating_add(cfg.viewport_lookahead).min(bytes_u32);
					let mut win_len = win_end.saturating_sub(win_start);

					if win_len > cfg.viewport_window_max {
						win_len = cfg.viewport_window_max;
						win_end = win_start.saturating_add(win_len).min(bytes_u32);
					}
					(cfg.viewport_injections, win_start, win_end)
				} else {
					// needs_stage_b
					b_latch_to_set = true;
					let range = win_override.as_ref().unwrap();
					(InjectionPolicy::Eager, range.start, range.end)
				}
			};

			if win_end > win_start {
				let class = TaskClass::Viewport;
				let parse_timeout =
					self.metrics
						.derive_timeout(lang_id, tier, class, injections, cfg.viewport_parse_timeout_min, cfg.viewport_parse_timeout_max);

				let entry = self.entry_mut(doc_id);
				let spec = TaskSpec {
					doc_id,
					epoch: entry.sched.epoch,
					doc_version: ctx.doc_version,
					lang_id,
					opts_key: current_opts_key,
					opts: SyntaxOptions { parse_timeout, injections },
					kind: TaskKind::ViewportParse {
						content: ctx.content.clone(),
						window: win_start..win_end,
					},
					loader: Arc::clone(ctx.loader),
				};

				let permits = Arc::clone(&self.permits);
				let engine = Arc::clone(&self.engine);

				if let Some(task_id) = self.collector.spawn(permits, engine, spec, self.cfg.viewport_reserve) {
					let entry = self.entries.get_mut(&doc_id).unwrap();
					entry.sched.active_task = Some(task_id);
					entry.sched.active_task_class = Some(class);
					entry.sched.requested_doc_version = ctx.doc_version;
					entry.sched.force_no_debounce = false;
					if b_latch_to_set {
						entry.slot.viewport_stage_b_attempted = true;
					}
					return SyntaxPollOutcome {
						result: SyntaxPollResult::Kicked,
						updated: was_updated,
					};
				}
			}
		}

		// 6b. Full or Incremental parse
		let (kind, class) = {
			let entry = self.entry_mut(doc_id);
			let incremental = match entry.slot.pending_incremental.as_ref() {
				Some(pending) if entry.slot.current.is_some() && entry.slot.tree_doc_version == Some(pending.base_tree_doc_version) => {
					Some(TaskKind::Incremental {
						base: entry.slot.current.as_ref().unwrap().clone(),
						old_rope: pending.old_rope.clone(),
						new_rope: ctx.content.clone(),
						composed: pending.composed.clone(),
					})
				}
				_ => None,
			};

			let kind = incremental.unwrap_or_else(|| TaskKind::FullParse { content: ctx.content.clone() });
			let class = kind.class();
			(kind, class)
		};

		let injections = cfg.injections;
		let parse_timeout = self
			.metrics
			.derive_timeout(lang_id, tier, class, injections, cfg.parse_timeout_min, cfg.parse_timeout_max);

		let entry = self.entry_mut(doc_id);
		let spec = TaskSpec {
			doc_id,
			epoch: entry.sched.epoch,
			doc_version: ctx.doc_version,
			lang_id,
			opts_key: current_opts_key,
			opts: SyntaxOptions { parse_timeout, injections },
			kind,
			loader: Arc::clone(ctx.loader),
		};

		let permits = Arc::clone(&self.permits);
		let engine = Arc::clone(&self.engine);

		if let Some(task_id) = self.collector.spawn(permits, engine, spec, self.cfg.viewport_reserve) {
			let entry = self.entry_mut(doc_id);
			entry.sched.active_task = Some(task_id);
			entry.sched.active_task_class = Some(class);
			entry.sched.requested_doc_version = ctx.doc_version;
			entry.sched.force_no_debounce = false;
			SyntaxPollOutcome {
				result: SyntaxPollResult::Kicked,
				updated: was_updated,
			}
		} else {
			SyntaxPollOutcome {
				result: SyntaxPollResult::Throttled,
				updated: was_updated,
			}
		}
	}
}
