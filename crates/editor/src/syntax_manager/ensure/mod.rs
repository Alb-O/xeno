use super::*;

/// Computes an aligned viewport key for cache reuse.
fn compute_viewport_key(viewport_start: u32, window_max: u32) -> ViewportKey {
	let stride = (window_max / 2).max(4096);
	let anchor = (viewport_start / stride) * stride;
	ViewportKey(anchor)
}

fn slot_has_eager_exact_viewport_tree_coverage(slot: &SyntaxSlot, viewport: &std::ops::Range<u32>, doc_version: u64) -> bool {
	let Some(key) = slot.viewport_cache.covering_key(viewport) else {
		return false;
	};
	let Some(cache_entry) = slot.viewport_cache.map.get(&key) else {
		return false;
	};
	let exact_covers = |t: &ViewportTree| t.doc_version == doc_version && t.coverage.start <= viewport.start && t.coverage.end >= viewport.end;
	cache_entry.stage_b.as_ref().is_some_and(&exact_covers)
		|| cache_entry
			.stage_a
			.as_ref()
			.is_some_and(|t| t.syntax.opts().injections == InjectionPolicy::Eager && exact_covers(t))
}

fn slot_has_stage_b_exact_viewport_coverage(slot: &SyntaxSlot, viewport: &std::ops::Range<u32>, doc_version: u64) -> bool {
	let Some(key) = slot.viewport_cache.covering_key(viewport) else {
		return false;
	};
	let Some(cache_entry) = slot.viewport_cache.map.get(&key) else {
		return false;
	};
	cache_entry
		.stage_b
		.as_ref()
		.is_some_and(|t| t.doc_version == doc_version && t.coverage.start <= viewport.start && t.coverage.end >= viewport.end)
}

impl SyntaxManager {
	/// Invariant enforcement: Polls or kicks background syntax parsing for a document.
	pub fn ensure_syntax(&mut self, ctx: EnsureSyntaxContext<'_>) -> SyntaxPollOutcome {
		self.ensure_syntax_at(Instant::now(), ctx)
	}

	/// Clock-injectable inner implementation of [`Self::ensure_syntax`].
	///
	/// Tests call this directly to deterministically advance time without sleeps.
	#[cfg_attr(not(test), inline(always))]
	pub(crate) fn ensure_syntax_at(&mut self, now: Instant, ctx: EnsureSyntaxContext<'_>) -> SyntaxPollOutcome {
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
		tracing::trace!(
			target: "xeno_undo_trace",
			?doc_id,
			doc_version = ctx.doc_version,
			bytes,
			?tier,
			hotness = ?ctx.hotness,
			language_id = ?ctx.language_id,
			viewport = ?viewport,
			work_disabled,
			"syntax.ensure.begin"
		);

		// 1. Initial entry check & change detection
		let mut was_updated = {
			let entry = self.entry_mut(doc_id);
			let mut updated = entry.slot.take_updated();

			if entry.slot.language_id != ctx.language_id {
				entry.sched.invalidate();
				if entry.slot.has_any_tree() {
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

			if work_disabled {
				entry.sched.active_viewport_urgent_detached = entry.sched.active_viewport_urgent.is_some();
				entry.sched.active_viewport_enrich_detached = entry.sched.active_viewport_enrich.is_some();
				entry.sched.active_bg_detached = entry.sched.active_bg.is_some();
			}

			updated
		};

		// 2. Process completed tasks (drain queue)
		// Collect metric records separately to avoid borrow conflicts.
		let mut metric_records: Vec<(
			xeno_language::LanguageId,
			SyntaxTier,
			TaskClass,
			InjectionPolicy,
			std::time::Duration,
			bool,
			bool,
			bool,
			bool,
		)> = Vec::new();
		{
			let entry = self.entry_mut(doc_id);
			while let Some(done) = entry.sched.completed.pop_front() {
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
								match done.viewport_lane {
									Some(scheduling::ViewportLane::Urgent) => injections == cfg.viewport_injections || injections == InjectionPolicy::Eager,
									Some(scheduling::ViewportLane::Enrich) => injections == InjectionPolicy::Eager && cfg.viewport_stage_b_budget.is_some(),
									None => false,
								}
							} else {
								done.opts == current_opts_key
							};
							let version_match = done.doc_version == ctx.doc_version;

							let is_viewport_task = class == TaskClass::Viewport;

							let retain_policy = if is_viewport_task {
								cfg.retention_hidden_viewport
							} else {
								cfg.retention_hidden_full
							};
							let retain_ok = Self::retention_allows_install(now, &entry.sched, retain_policy, ctx.hotness);

							let allow_install = if is_viewport_task {
								let not_future = done.doc_version <= ctx.doc_version;
								let requested_min = match done.viewport_lane {
									Some(scheduling::ViewportLane::Enrich) => entry.sched.requested_viewport_enrich_doc_version,
									_ => entry.sched.requested_viewport_urgent_doc_version,
								};
								let requested_ok = done.doc_version >= requested_min;
								let continuity_needed = if done.doc_version < ctx.doc_version {
									match &viewport {
										Some(vp) => {
											let has_covering_tree = entry.slot.full.is_some() || entry.slot.viewport_cache.covers_range(vp);
											!has_covering_tree
										}
										None => true,
									}
								} else {
									true
								};
								not_future && requested_ok && continuity_needed
							} else {
								let preserves_projection_continuity = if done.doc_version < ctx.doc_version {
									if entry.slot.full.is_none() {
										true
									} else {
										entry
											.slot
											.pending_incremental
											.as_ref()
											.is_some_and(|p| p.base_tree_doc_version == done.doc_version)
									}
								} else {
									true
								};
								Self::should_install_completed_parse(
									done.doc_version,
									entry.slot.full_doc_version,
									entry.sched.requested_bg_doc_version,
									ctx.doc_version,
									entry.slot.dirty,
								) && preserves_projection_continuity
							};

							let is_installed = if work_disabled {
								false
							} else if lang_ok && opts_ok && retain_ok && allow_install {
								if is_viewport_task {
									if let Some(vp_key) = done.viewport_key {
										let tree_id = entry.slot.alloc_tree_id();
										let coverage = if let Some(meta) = &syntax_tree.viewport {
											meta.base_offset..meta.base_offset.saturating_add(meta.real_len)
										} else {
											0..0
										};

										let vp_tree = ViewportTree {
											syntax: syntax_tree,
											doc_version: done.doc_version,
											tree_id,
											coverage,
										};

										let cache_entry = entry.slot.viewport_cache.get_mut_or_insert(vp_key);
										let is_stage_a = matches!(done.viewport_lane, Some(scheduling::ViewportLane::Urgent));
										if is_stage_a {
											cache_entry.stage_a = Some(vp_tree);
											cache_entry.stage_a_failed_for = None;
											cache_entry.attempted_b_for = None;
											entry.slot.dirty = true;
											entry.sched.force_no_debounce = true;
										} else {
											cache_entry.stage_b = Some(vp_tree);
											cache_entry.stage_b_cooldown_until = None;
										}
										entry.slot.language_id = Some(current_lang);
									}
								} else {
									let tree_id = entry.slot.alloc_tree_id();
									entry.slot.full = Some(syntax_tree);
									entry.slot.full_doc_version = Some(done.doc_version);
									entry.slot.full_tree_id = tree_id;
									entry.slot.language_id = Some(current_lang);
									let keep_pending_projection = done.doc_version < ctx.doc_version
										&& entry
											.slot
											.pending_incremental
											.as_ref()
											.is_some_and(|p| p.base_tree_doc_version == done.doc_version);
									if !keep_pending_projection {
										entry.slot.pending_incremental = None;
									}

									entry.sched.force_no_debounce = false;
									if version_match {
										entry.slot.dirty = false;
										entry.sched.cooldown_until = None;
									} else {
										entry.slot.dirty = true;
									}
								}

								Self::mark_updated(&mut entry.slot);
								was_updated = true;
								true
							} else {
								if lang_ok && opts_ok && version_match && !retain_ok {
									entry.slot.drop_tree();
									entry.slot.dirty = false;
									entry.sched.force_no_debounce = false;
									Self::mark_updated(&mut entry.slot);
									was_updated = true;
								}
								false
							};

							metric_records.push((lang_id, tier, class, injections, elapsed, is_timeout, is_error, is_installed, false));
							tracing::trace!(
								target: "xeno_undo_trace",
								?doc_id,
								done_doc_version = done.doc_version,
								ctx_doc_version = ctx.doc_version,
								?class,
								viewport_lane = ?done.viewport_lane,
								?injections,
								elapsed_ms = elapsed.as_millis() as u64,
								is_installed,
								"syntax.ensure.completed.ok"
							);
						} else {
							metric_records.push((lang_id, tier, class, injections, elapsed, is_timeout, is_error, false, false));
							tracing::trace!(
								target: "xeno_undo_trace",
								?doc_id,
								done_doc_version = done.doc_version,
								ctx_doc_version = ctx.doc_version,
								?class,
								viewport_lane = ?done.viewport_lane,
								?injections,
								elapsed_ms = elapsed.as_millis() as u64,
								is_installed = false,
								result = "no_language",
								"syntax.ensure.completed.ok"
							);
						}
					}
					Err(xeno_language::syntax::SyntaxError::Timeout) => {
						let is_enrich = done.viewport_lane == Some(scheduling::ViewportLane::Enrich);
						if is_enrich {
							// Per-key cooldown only; don't block other lanes
							if let Some(vp_key) = done.viewport_key {
								let ce = entry.slot.viewport_cache.get_mut_or_insert(vp_key);
								ce.stage_b_cooldown_until = Some(now + cfg.viewport_cooldown_on_timeout);
								ce.attempted_b_for = None;
							}
						} else {
							if done.viewport_lane == Some(scheduling::ViewportLane::Urgent)
								&& let Some(vp_key) = done.viewport_key
							{
								let ce = entry.slot.viewport_cache.get_mut_or_insert(vp_key);
								ce.stage_a_failed_for = Some(done.doc_version);
							}
							let cooldown = if class == TaskClass::Viewport {
								cfg.viewport_cooldown_on_timeout
							} else {
								cfg.cooldown_on_timeout
							};
							entry.sched.cooldown_until = Some(now + cooldown);
						}
						metric_records.push((lang_id, tier, class, injections, elapsed, true, false, false, is_enrich));
						tracing::trace!(
							target: "xeno_undo_trace",
							?doc_id,
							done_doc_version = done.doc_version,
							ctx_doc_version = ctx.doc_version,
							?class,
							viewport_lane = ?done.viewport_lane,
							?injections,
							elapsed_ms = elapsed.as_millis() as u64,
							is_enrich,
							"syntax.ensure.completed.timeout"
						);
					}
					Err(e) => {
						tracing::warn!(?doc_id, ?tier, error=%e, "Background syntax parse failed");
						let is_enrich = done.viewport_lane == Some(scheduling::ViewportLane::Enrich);
						if is_enrich {
							if let Some(vp_key) = done.viewport_key {
								let ce = entry.slot.viewport_cache.get_mut_or_insert(vp_key);
								ce.stage_b_cooldown_until = Some(now + cfg.viewport_cooldown_on_error);
								ce.attempted_b_for = None;
							}
						} else {
							if done.viewport_lane == Some(scheduling::ViewportLane::Urgent)
								&& let Some(vp_key) = done.viewport_key
							{
								let ce = entry.slot.viewport_cache.get_mut_or_insert(vp_key);
								ce.stage_a_failed_for = Some(done.doc_version);
							}
							let cooldown = if class == TaskClass::Viewport {
								cfg.viewport_cooldown_on_error
							} else {
								cfg.cooldown_on_error
							};
							entry.sched.cooldown_until = Some(now + cooldown);
						}
						metric_records.push((lang_id, tier, class, injections, elapsed, false, true, false, is_enrich));
						tracing::trace!(
							target: "xeno_undo_trace",
							?doc_id,
							done_doc_version = done.doc_version,
							ctx_doc_version = ctx.doc_version,
							?class,
							viewport_lane = ?done.viewport_lane,
							?injections,
							elapsed_ms = elapsed.as_millis() as u64,
							error = %e,
							is_enrich,
							"syntax.ensure.completed.error"
						);
					}
				}
			}
		}

		let mut any_blocking_timeout_or_error = false;
		for (lang_id, tier, class, injections, elapsed, is_timeout, is_error, is_installed, is_enrich) in metric_records {
			self.metrics
				.record_task_result(lang_id, tier, class, injections, elapsed, is_timeout, is_error, is_installed);
			if (is_timeout || is_error) && !is_enrich {
				any_blocking_timeout_or_error = true;
			}
		}

		if any_blocking_timeout_or_error {
			tracing::trace!(
				target: "xeno_undo_trace",
				?doc_id,
				doc_version = ctx.doc_version,
				updated = was_updated,
				"syntax.ensure.return.cooldown_blocking_failure"
			);
			return SyntaxPollOutcome {
				result: SyntaxPollResult::CoolingDown,
				updated: was_updated,
			};
		}

		let mut viewport_stable_polls: u8 = 0;
		let mut want_enrich = false;
		let mut viewport_uncovered = false;

		{
			let entry = self.entry_mut(doc_id);

			// Retention
			if entry.sched.any_active() {
				if Self::apply_retention(
					now,
					&entry.sched,
					cfg.retention_hidden_full,
					cfg.retention_hidden_viewport,
					ctx.hotness,
					&mut entry.slot,
					doc_id,
				) {
					entry.sched.invalidate();
					was_updated = true;
				}
			}

			// 4. Language check
			let Some(_lang_id) = ctx.language_id else {
				if entry.slot.has_any_tree() {
					entry.slot.drop_tree();
					Self::mark_updated(&mut entry.slot);
					was_updated = true;
				}
				entry.slot.language_id = None;
				entry.slot.dirty = false;
				entry.sched.cooldown_until = None;
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					updated = was_updated,
					"syntax.ensure.return.no_language"
				);
				return SyntaxPollOutcome {
					result: SyntaxPollResult::NoLanguage,
					updated: was_updated,
				};
			};

			if Self::apply_retention(
				now,
				&entry.sched,
				cfg.retention_hidden_full,
				cfg.retention_hidden_viewport,
				ctx.hotness,
				&mut entry.slot,
				doc_id,
			) {
				if !work_disabled {
					entry.sched.invalidate();
				}
				was_updated = true;
			}

			// 5. Gating
			if work_disabled {
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					updated = was_updated,
					"syntax.ensure.return.disabled"
				);
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Disabled,
					updated: was_updated,
				};
			}

			// Reattach detached tasks when becoming visible again
			if entry.sched.active_viewport_urgent_detached {
				entry.sched.active_viewport_urgent_detached = false;
			}
			if entry.sched.active_viewport_enrich_detached {
				entry.sched.active_viewport_enrich_detached = false;
			}
			if entry.sched.active_bg_detached {
				entry.sched.active_bg_detached = false;
			}

			// Track viewport focus stability for Stage-B gating.
			// Use covering_key so that stride-boundary viewport shifts don't reset
			// stability when the enrichment target key hasn't actually changed.
			viewport_stable_polls = if let Some(vp) = &viewport {
				let focus_key = entry
					.slot
					.viewport_cache
					.covering_key(vp)
					.unwrap_or_else(|| compute_viewport_key(vp.start, cfg.viewport_window_max));
				entry.sched.note_viewport_focus(focus_key, ctx.doc_version)
			} else {
				0
			};

			// MRU touch: keep the current viewport key hot in the cache
			if let Some(vp) = &viewport {
				if let Some(covering) = entry.slot.viewport_cache.covering_key(vp) {
					entry.slot.viewport_cache.touch(covering);
				} else {
					let key = compute_viewport_key(vp.start, cfg.viewport_window_max);
					if entry.slot.viewport_cache.map.contains_key(&key) {
						entry.slot.viewport_cache.touch(key);
					}
				}
			}

			// Compute enrichment desire using covering key (not just computed key)
			want_enrich = tier == SyntaxTier::L && ctx.hotness == SyntaxHotness::Visible && cfg.viewport_stage_b_budget.is_some() && viewport.is_some() && {
				let vp = viewport.as_ref().unwrap();
				!slot_has_stage_b_exact_viewport_coverage(&entry.slot, vp, ctx.doc_version)
			};

			viewport_uncovered =
				tier == SyntaxTier::L && entry.slot.full.is_none() && viewport.as_ref().is_some_and(|vp| !entry.slot.viewport_cache.covers_range(vp));

			if entry.slot.has_any_tree() && !entry.slot.dirty && !want_enrich && !viewport_uncovered {
				entry.sched.force_no_debounce = false;
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					updated = was_updated,
					"syntax.ensure.return.ready_fast_path"
				);
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Ready,
					updated: was_updated,
				};
			}

			if entry.slot.has_any_tree() && !entry.sched.force_no_debounce && now.duration_since(entry.sched.last_edit_at) < cfg.debounce {
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					updated = was_updated,
					force_no_debounce = entry.sched.force_no_debounce,
					"syntax.ensure.return.pending_debounce"
				);
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Pending,
					updated: was_updated,
				};
			}

			if let Some(until) = entry.sched.cooldown_until
				&& now < until
			{
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					updated = was_updated,
					remaining_ms = until.saturating_duration_since(now).as_millis() as u64,
					"syntax.ensure.return.cooling_down"
				);
				return SyntaxPollOutcome {
					result: SyntaxPollResult::CoolingDown,
					updated: was_updated,
				};
			}
		}

		// 5.5 Sync bootstrap fast path
		let lang_id = ctx.language_id.unwrap();
		let (do_sync, sync_timeout, pre_epoch) = {
			let entry = self.entry_mut(doc_id);
			let is_bootstrap = !entry.slot.has_any_tree();
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
			tracing::trace!(
				target: "xeno_undo_trace",
				?doc_id,
				doc_version = ctx.doc_version,
				sync_timeout_ms = sync_opts.parse_timeout.as_millis() as u64,
				injections = ?sync_opts.injections,
				"syntax.ensure.sync_bootstrap.attempt"
			);
			Some(self.engine.parse(ctx.content.slice(..), lang_id, ctx.loader, sync_opts))
		} else {
			None
		};

		match sync_result {
			Some(Ok(syntax)) => {
				let entry = self.entry_mut(doc_id);
				let is_bootstrap = !entry.slot.has_any_tree();
				let is_visible = matches!(ctx.hotness, SyntaxHotness::Visible);
				if entry.sched.epoch == pre_epoch && is_bootstrap && is_visible && !entry.sched.any_active() {
					let tree_id = entry.slot.alloc_tree_id();
					entry.slot.full = Some(syntax);
					entry.slot.full_doc_version = Some(ctx.doc_version);
					entry.slot.full_tree_id = tree_id;
					entry.slot.language_id = Some(lang_id);
					entry.slot.dirty = false;
					entry.slot.pending_incremental = None;
					entry.sched.force_no_debounce = false;
					entry.sched.cooldown_until = None;
					Self::mark_updated(&mut entry.slot);
					tracing::trace!(
						target: "xeno_undo_trace",
						?doc_id,
						doc_version = ctx.doc_version,
						tree_id,
						"syntax.ensure.sync_bootstrap.installed"
					);
					return SyntaxPollOutcome {
						result: SyntaxPollResult::Ready,
						updated: true,
					};
				}
			}
			Some(Err(e)) => {
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					error = %e,
					"syntax.ensure.sync_bootstrap.failed"
				);
			}
			None => {}
		}

		// 6. Schedule new tasks
		let mut kicked_any = false;

		// 6a. Viewport urgent lane: Stage-A
		let (needs_stage_a, stage_a_key) = {
			let entry = self.entry_mut(doc_id);
			if tier == SyntaxTier::L
				&& ctx.hotness == SyntaxHotness::Visible
				&& let Some(viewport) = &viewport
				&& !entry.sched.viewport_urgent_active()
			{
				let viewport_uncovered = entry.slot.full.is_none() && !entry.slot.viewport_cache.covers_range(viewport);
				let viewport_key = compute_viewport_key(viewport.start, cfg.viewport_window_max);
				let history_stage_a_failed_for_doc_version = entry.slot.viewport_cache.map.get(&viewport_key).and_then(|ce| ce.stage_a_failed_for);
				let history_needs_eager_repair = entry.sched.last_edit_source == EditSource::History
					&& !slot_has_eager_exact_viewport_tree_coverage(&entry.slot, viewport, ctx.doc_version)
					&& history_stage_a_failed_for_doc_version != Some(ctx.doc_version);
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					viewport = ?viewport,
					viewport_key = viewport_key.0,
					viewport_uncovered,
					history_needs_eager_repair,
					history_stage_a_failed_for_doc_version,
					last_edit_source = ?entry.sched.last_edit_source,
					"syntax.ensure.stage_a.decide"
				);
				if viewport_uncovered || history_needs_eager_repair {
					(true, Some(viewport_key))
				} else {
					(false, None)
				}
			} else {
				(false, None)
			}
		};

		if needs_stage_a {
			let viewport_key = stage_a_key.unwrap();
			let viewport = viewport.as_ref().unwrap();
			let win_start = viewport_key.0.saturating_sub(cfg.viewport_lookbehind);
			let mut win_end = viewport_key.0.saturating_add(cfg.viewport_window_max).min(bytes_u32);
			win_end = win_end.max(viewport.end.saturating_add(cfg.viewport_lookahead).min(bytes_u32));
			let mut win_len = win_end.saturating_sub(win_start);
			if win_len > cfg.viewport_window_max {
				win_len = cfg.viewport_window_max;
				win_end = win_start.saturating_add(win_len).min(bytes_u32);
			}

			if win_end > win_start {
				let class = TaskClass::Viewport;
				let (injections, history_urgent) = {
					let entry = self.entry_mut(doc_id);
					let history_urgent = tier == SyntaxTier::L && entry.sched.last_edit_source == EditSource::History;
					let injections = if history_urgent { InjectionPolicy::Eager } else { cfg.viewport_injections };
					(injections, history_urgent)
				};
				let mut parse_timeout =
					self.metrics
						.derive_timeout(lang_id, tier, class, injections, cfg.viewport_parse_timeout_min, cfg.viewport_parse_timeout_max);
				if history_urgent {
					let history_floor = cfg.viewport_parse_timeout_max * 3;
					parse_timeout = parse_timeout.max(history_floor);
				}

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
						key: viewport_key,
					},
					loader: Arc::clone(ctx.loader),
					viewport_key: Some(viewport_key),
					viewport_lane: Some(scheduling::ViewportLane::Urgent),
				};

				let permits = Arc::clone(&self.permits);
				let engine = Arc::clone(&self.engine);

				if let Some(task_id) = self.collector.spawn(permits, engine, spec, self.cfg.viewport_reserve, true) {
					let entry = self.entries.get_mut(&doc_id).unwrap();
					entry.sched.active_viewport_urgent = Some(task_id);
					entry.sched.requested_viewport_urgent_doc_version = ctx.doc_version;
					kicked_any = true;
					tracing::trace!(
						target: "xeno_undo_trace",
						?doc_id,
						doc_version = ctx.doc_version,
						task_id = task_id.0,
						viewport_key = viewport_key.0,
						win_start,
						win_end,
						injections = ?injections,
						parse_timeout_ms = parse_timeout.as_millis() as u64,
						"syntax.ensure.stage_a.spawned"
					);
				} else {
					tracing::trace!(
						target: "xeno_undo_trace",
						?doc_id,
						doc_version = ctx.doc_version,
						viewport_key = viewport_key.0,
						win_start,
						win_end,
						injections = ?injections,
						parse_timeout_ms = parse_timeout.as_millis() as u64,
						"syntax.ensure.stage_a.spawn_rejected"
					);
				}
			}
		}

		// 6b. Viewport enrich lane: Stage-B
		let (needs_stage_b, stage_b_key, stage_b_win) = {
			let entry = self.entry_mut(doc_id);
			if tier == SyntaxTier::L
				&& ctx.hotness == SyntaxHotness::Visible
				&& let Some(viewport) = &viewport
				&& !entry.sched.viewport_enrich_active()
				&& !entry.sched.viewport_urgent_active()
				&& cfg.viewport_stage_b_budget.is_some()
				&& viewport_stable_polls >= cfg.viewport_stage_b_min_stable_polls
			{
				let k = entry
					.slot
					.viewport_cache
					.covering_key(viewport)
					.unwrap_or_else(|| compute_viewport_key(viewport.start, cfg.viewport_window_max));
				let cache_entry = entry.slot.viewport_cache.map.get(&k);
				let eager_covers = slot_has_stage_b_exact_viewport_coverage(&entry.slot, viewport, ctx.doc_version);
				let already_attempted = cache_entry.is_some_and(|ce| ce.attempted_b_for == Some(ctx.doc_version));
				let in_cooldown = cache_entry.is_some_and(|ce| ce.stage_b_cooldown_until.is_some_and(|until| now < until));
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					viewport = ?viewport,
					viewport_key = k.0,
					eager_covers,
					already_attempted,
					in_cooldown,
					viewport_stable_polls,
					"syntax.ensure.stage_b.decide"
				);
				if !eager_covers && !already_attempted && !in_cooldown {
					let win = cache_entry.and_then(|ce| ce.stage_a.as_ref().map(|sa| sa.coverage.clone()));
					(true, Some(k), win)
				} else {
					(false, None, None)
				}
			} else {
				(false, None, None)
			}
		};

		if needs_stage_b {
			let budget = cfg.viewport_stage_b_budget.unwrap();
			let predicted = self.metrics.predict_duration(lang_id, tier, TaskClass::Viewport, InjectionPolicy::Eager);
			let within_budget = predicted.map(|p| p <= budget).unwrap_or(true);
			tracing::trace!(
				target: "xeno_undo_trace",
				?doc_id,
				doc_version = ctx.doc_version,
				budget_ms = budget.as_millis() as u64,
				predicted_ms = predicted.map(|p| p.as_millis() as u64),
				within_budget,
				"syntax.ensure.stage_b.budget"
			);

			if within_budget {
				let viewport_key = stage_b_key.unwrap();
				let viewport = viewport.as_ref().unwrap();
				let (win_start, win_end) = if let Some(range) = stage_b_win.as_ref() {
					(range.start, range.end)
				} else {
					let win_start = viewport_key.0.saturating_sub(cfg.viewport_lookbehind);
					let mut win_end = viewport.end.saturating_add(cfg.viewport_lookahead).min(bytes_u32);
					let mut win_len = win_end.saturating_sub(win_start);
					if win_len > cfg.viewport_window_max {
						win_len = cfg.viewport_window_max;
						win_end = win_start.saturating_add(win_len).min(bytes_u32);
					}
					(win_start, win_end)
				};

				if win_end > win_start {
					let class = TaskClass::Viewport;
					let injections = InjectionPolicy::Eager;
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
							key: viewport_key,
						},
						loader: Arc::clone(ctx.loader),
						viewport_key: Some(viewport_key),
						viewport_lane: Some(scheduling::ViewportLane::Enrich),
					};

					let permits = Arc::clone(&self.permits);
					let engine = Arc::clone(&self.engine);

					if let Some(task_id) = self.collector.spawn(permits, engine, spec, self.cfg.viewport_reserve, false) {
						let entry = self.entries.get_mut(&doc_id).unwrap();
						entry.sched.active_viewport_enrich = Some(task_id);
						entry.sched.requested_viewport_enrich_doc_version = ctx.doc_version;
						let ce = entry.slot.viewport_cache.get_mut_or_insert(viewport_key);
						ce.attempted_b_for = Some(ctx.doc_version);
						ce.stage_b_cooldown_until = None;
						kicked_any = true;
						tracing::trace!(
							target: "xeno_undo_trace",
							?doc_id,
							doc_version = ctx.doc_version,
							task_id = task_id.0,
							viewport_key = viewport_key.0,
							win_start,
							win_end,
							injections = ?injections,
							parse_timeout_ms = parse_timeout.as_millis() as u64,
							"syntax.ensure.stage_b.spawned"
						);
					} else {
						tracing::trace!(
							target: "xeno_undo_trace",
							?doc_id,
							doc_version = ctx.doc_version,
							viewport_key = viewport_key.0,
							win_start,
							win_end,
							injections = ?injections,
							parse_timeout_ms = parse_timeout.as_millis() as u64,
							"syntax.ensure.stage_b.spawn_rejected"
						);
					}
				}
			} else {
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					budget_ms = budget.as_millis() as u64,
					predicted_ms = predicted.map(|p| p.as_millis() as u64),
					"syntax.ensure.stage_b.skipped_budget"
				);
			}
		}

		// 6c. Background lane: Full or Incremental parse
		let bg_needed = {
			let entry = self.entry_mut(doc_id);
			(entry.slot.dirty || entry.slot.full.is_none()) && !entry.sched.bg_active()
		};

		if bg_needed {
			let (kind, class) = {
				let entry = self.entry_mut(doc_id);
				let incremental = match entry.slot.pending_incremental.as_ref() {
					Some(pending) if entry.slot.full.is_some() && entry.slot.full_doc_version == Some(pending.base_tree_doc_version) => {
						Some(TaskKind::Incremental {
							base: entry.slot.full.as_ref().unwrap().clone(),
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
				viewport_key: None,
				viewport_lane: None,
			};

			let permits = Arc::clone(&self.permits);
			let engine = Arc::clone(&self.engine);

			if let Some(task_id) = self.collector.spawn(permits, engine, spec, self.cfg.viewport_reserve, false) {
				let entry = self.entry_mut(doc_id);
				entry.sched.active_bg = Some(task_id);
				entry.sched.requested_bg_doc_version = ctx.doc_version;
				entry.sched.force_no_debounce = false;
				kicked_any = true;
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					task_id = task_id.0,
					?class,
					injections = ?injections,
					parse_timeout_ms = parse_timeout.as_millis() as u64,
					"syntax.ensure.background.spawned"
				);
			} else {
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					?class,
					injections = ?injections,
					parse_timeout_ms = parse_timeout.as_millis() as u64,
					"syntax.ensure.background.spawn_rejected"
				);
			}
		}

		if kicked_any {
			tracing::trace!(
				target: "xeno_undo_trace",
				?doc_id,
				doc_version = ctx.doc_version,
				updated = was_updated,
				"syntax.ensure.return.kicked"
			);
			SyntaxPollOutcome {
				result: SyntaxPollResult::Kicked,
				updated: was_updated,
			}
		} else {
			let entry = self.entry_mut(doc_id);
			let desired_work = entry.slot.dirty || entry.slot.full.is_none() || viewport_uncovered || want_enrich;
			if entry.sched.any_active() || desired_work {
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					updated = was_updated,
					desired_work,
					active = entry.sched.any_active(),
					"syntax.ensure.return.pending"
				);
				SyntaxPollOutcome {
					result: SyntaxPollResult::Pending,
					updated: was_updated,
				}
			} else {
				tracing::trace!(
					target: "xeno_undo_trace",
					?doc_id,
					doc_version = ctx.doc_version,
					updated = was_updated,
					desired_work,
					active = entry.sched.any_active(),
					"syntax.ensure.return.ready"
				);
				SyntaxPollOutcome {
					result: SyntaxPollResult::Ready,
					updated: was_updated,
				}
			}
		}
	}
}

#[cfg(test)]
mod tests;
