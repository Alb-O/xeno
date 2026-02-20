use super::*;

impl CommandPaletteOverlay {
	pub(super) fn build_command_items(query: &str, usage: &crate::completion::CommandUsageSnapshot) -> Vec<CompletionItem> {
		let query = query.trim();
		let mut scored: Vec<(bool, i32, CompletionItem)> = COMMANDS
			.snapshot_guard()
			.iter_refs()
			.filter_map(|cmd| {
				let name = cmd.name_str();
				let description = cmd.description_str();
				let mut best_score = i32::MIN;
				let mut exact_alias_match = false;
				let mut match_indices: Option<Vec<usize>> = None;

				if let Some((score, _, indices)) = crate::completion::frizbee_match(query, name) {
					best_score = score as i32 + 220;
					if !indices.is_empty() {
						match_indices = Some(indices);
					}
				}

				for alias in cmd.keys_resolved() {
					if let Some((score, _, _)) = crate::completion::frizbee_match(query, alias) {
						best_score = best_score.max(score as i32 + 80);
						if !alias.eq_ignore_ascii_case(name) && alias.eq_ignore_ascii_case(query) {
							exact_alias_match = true;
						}
					}
				}

				if let Some((score, _, _)) = crate::completion::frizbee_match(query, description) {
					best_score = best_score.max(score as i32 - 120);
				}

				if query.is_empty() {
					best_score = 0;
				}

				if !query.is_empty() && best_score == i32::MIN {
					return None;
				}

				let right = cmd
					.keys_resolved()
					.iter()
					.find(|alias| **alias != name && alias.len() <= 8)
					.map(|alias| alias.to_string());

				let count = usage.count(name);
				let frequency_bonus = if count == 0 { 0 } else { (31 - (count + 1).leading_zeros()) as i32 * 40 };
				let recency_bonus = if query.chars().count() <= 2 {
					usage.recent_rank(name).map_or(0, |rank| (120i32 - (rank as i32 * 12)).max(0))
				} else {
					0
				};

				Some((
					exact_alias_match,
					best_score + frequency_bonus + recency_bonus,
					CompletionItem {
						label: name.to_string(),
						insert_text: name.to_string(),
						detail: Some(description.to_string()),
						filter_text: None,
						kind: CompletionKind::Command,
						match_indices,
						right,
						file: None,
					},
				))
			})
			.collect();

		if !scored.iter().any(|(_, _, item)| item.label == "files") {
			let mut best_score = i32::MIN;
			let mut exact_alias_match = false;
			let mut match_indices = None;

			if let Some((score, _, indices)) = crate::completion::frizbee_match(query, "files") {
				best_score = score as i32 + 220;
				if !indices.is_empty() {
					match_indices = Some(indices);
				}
			}

			if let Some((score, _, _)) = crate::completion::frizbee_match(query, "fp") {
				best_score = best_score.max(score as i32 + 80);
				if query.eq_ignore_ascii_case("fp") {
					exact_alias_match = true;
				}
			}

			if let Some((score, _, _)) = crate::completion::frizbee_match(query, "Open file picker") {
				best_score = best_score.max(score as i32 - 120);
			}

			if query.is_empty() {
				best_score = 0;
			}

			if query.is_empty() || best_score != i32::MIN {
				let count = usage.count("files");
				let frequency_bonus = if count == 0 { 0 } else { (31 - (count + 1).leading_zeros()) as i32 * 40 };
				let recency_bonus = if query.chars().count() <= 2 {
					usage.recent_rank("files").map_or(0, |rank| (120i32 - (rank as i32 * 12)).max(0))
				} else {
					0
				};

				scored.push((
					exact_alias_match,
					best_score + frequency_bonus + recency_bonus,
					CompletionItem {
						label: "files".to_string(),
						insert_text: "files".to_string(),
						detail: Some("Open file picker".to_string()),
						filter_text: None,
						kind: CompletionKind::Command,
						match_indices,
						right: Some("fp".to_string()),
						file: None,
					},
				));
			}
		}

		if query.is_empty() {
			scored.sort_by(|(_, score_a, item_a), (_, score_b, item_b)| {
				let recent_a = usage.recent_rank(&item_a.label).unwrap_or(usize::MAX);
				let recent_b = usage.recent_rank(&item_b.label).unwrap_or(usize::MAX);
				recent_a
					.cmp(&recent_b)
					.then_with(|| score_b.cmp(score_a))
					.then_with(|| item_a.label.cmp(&item_b.label))
			});
		} else {
			scored.sort_by(|(exact_a, score_a, item_a), (exact_b, score_b, item_b)| {
				exact_b
					.cmp(exact_a)
					.then_with(|| score_b.cmp(score_a))
					.then_with(|| item_a.label.cmp(&item_b.label))
			});
		}

		scored.into_iter().map(|(_, _, item)| item).collect()
	}

	pub(super) fn build_theme_items(query: &str) -> Vec<CompletionItem> {
		let query = query.trim();
		let mut scored: Vec<(i32, CompletionItem)> = THEMES
			.snapshot_guard()
			.iter_refs()
			.filter_map(|theme| {
				let name = theme.name_str();
				let mut best_score = i32::MIN;
				let mut match_indices: Option<Vec<usize>> = None;

				if let Some((score, _, indices)) = crate::completion::frizbee_match(query, name) {
					best_score = score as i32 + 200;
					if !indices.is_empty() {
						match_indices = Some(indices);
					}
				}

				for alias in theme.keys_resolved() {
					if let Some((score, _, _)) = crate::completion::frizbee_match(query, alias) {
						best_score = best_score.max(score as i32 + 70);
					}
				}

				if query.is_empty() {
					best_score = 0;
				}

				if !query.is_empty() && best_score == i32::MIN {
					return None;
				}

				let variant = match theme.variant {
					ThemeVariant::Dark => "dark",
					ThemeVariant::Light => "light",
				};

				Some((
					best_score,
					CompletionItem {
						label: name.to_string(),
						insert_text: name.to_string(),
						detail: Some(format!("{variant} theme")),
						filter_text: None,
						kind: CompletionKind::Theme,
						match_indices,
						right: Some(variant.to_string()),
						file: None,
					},
				))
			})
			.collect();

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));

		scored.into_iter().map(|(_, item)| item).collect()
	}

	pub(super) fn command_arg_spec(command_name: &str, token_index: usize) -> Option<xeno_registry::commands::PaletteArgSpec> {
		if token_index == 0 {
			return None;
		}

		let arg_index = token_index.saturating_sub(1);
		let cmd = xeno_registry::commands::find_command(command_name)?;
		let args = &cmd.palette().args;
		if let Some(spec) = args.get(arg_index) {
			return Some(spec.clone());
		}

		args.last().filter(|last| last.variadic).cloned()
	}

	pub(super) fn command_arg_completion(command_name: &str, token_index: usize) -> CommandArgCompletion {
		Self::command_arg_spec(command_name, token_index)
			.map(|spec| CommandArgCompletion::from_palette_kind(spec.kind))
			.unwrap_or(CommandArgCompletion::None)
	}

	pub(super) fn command_supports_argument_completion(command_name: &str) -> bool {
		Self::command_arg_completion(command_name, 1).supports_completion()
	}

	pub(super) fn command_requires_argument_for_commit(command_name: &str) -> bool {
		xeno_registry::commands::find_command(command_name)
			.map(|cmd| cmd.palette().commit_policy == PaletteCommitPolicy::RequireResolvedArgs)
			.unwrap_or(false)
	}

	pub(super) fn should_append_space_after_completion(selected: &CompletionItem, token: &TokenCtx, is_dir_completion: bool, quoted_arg: bool) -> bool {
		match selected.kind {
			CompletionKind::Command => {
				if token.token_index == 0 {
					Self::command_supports_argument_completion(&selected.insert_text)
				} else {
					true
				}
			}
			CompletionKind::File => !is_dir_completion && !quoted_arg,
			CompletionKind::Snippet | CompletionKind::Theme => true,
			CompletionKind::Buffer => false,
		}
	}

	pub(super) fn build_snippet_items(query: &str) -> Vec<CompletionItem> {
		let query = query.trim();
		let query = query.strip_prefix('@').unwrap_or(query);
		let mut scored: Vec<(i32, CompletionItem)> = SNIPPETS
			.snapshot_guard()
			.iter_refs()
			.filter_map(|snippet| {
				let name = snippet.name_str();
				let label = format!("@{name}");
				let mut best_score = i32::MIN;
				let mut match_indices: Option<Vec<usize>> = None;

				if let Some((score, _, indices)) = crate::completion::frizbee_match(query, name) {
					best_score = score as i32 + 220;
					if !indices.is_empty() {
						match_indices = Some(indices.into_iter().map(|idx| idx.saturating_add(1)).collect());
					}
				}

				for alias in snippet.keys_resolved() {
					if let Some((score, _, _)) = crate::completion::frizbee_match(query, alias) {
						best_score = best_score.max(score as i32 + 80);
					}
				}

				if query.is_empty() {
					best_score = 0;
				}

				if !query.is_empty() && best_score == i32::MIN {
					return None;
				}

				Some((
					best_score,
					CompletionItem {
						label: label.clone(),
						insert_text: label,
						detail: Some(snippet.description_str().to_string()),
						filter_text: None,
						kind: CompletionKind::Snippet,
						match_indices,
						right: None,
						file: None,
					},
				))
			})
			.collect();

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));

		scored.into_iter().map(|(_, item)| item).collect()
	}

	pub(super) fn file_completion_base_dir(ctx: &dyn OverlayContext, session: &OverlaySession) -> PathBuf {
		ctx.buffer(session.origin_view)
			.and_then(|buffer| buffer.path())
			.and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
			.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
	}

	pub(super) fn cached_dir_entries(&mut self, dir_path: &Path) -> Vec<(String, bool)> {
		if let Some((cached_path, cached_entries)) = &self.file_cache
			&& cached_path == dir_path
		{
			return cached_entries.clone();
		}

		let entries = fs::read_dir(dir_path)
			.ok()
			.into_iter()
			.flatten()
			.flatten()
			.take(200)
			.map(|entry| {
				let label = entry.file_name().to_string_lossy().to_string();
				let is_dir = entry.file_type().ok().is_some_and(|ft| ft.is_dir());
				(label, is_dir)
			})
			.collect::<Vec<_>>();

		self.file_cache = Some((dir_path.to_path_buf(), entries.clone()));
		entries
	}

	pub(super) fn build_file_items(&mut self, query: &str, dir_part: Option<&str>, ctx: &dyn OverlayContext, session: &OverlaySession) -> Vec<CompletionItem> {
		let base_dir = Self::file_completion_base_dir(ctx, session);
		let dir_path = if let Some(dir_part) = dir_part {
			let part = PathBuf::from(dir_part);
			if part.is_absolute() { part } else { base_dir.join(part) }
		} else {
			base_dir
		};

		let mut scored = Vec::new();
		for (label, is_dir) in self.cached_dir_entries(&dir_path) {
			if !query.starts_with('.') && label.starts_with('.') {
				continue;
			}

			let Some((score, _, indices)) = crate::completion::frizbee_match(query, &label) else {
				continue;
			};

			let insert_text = if is_dir { format!("{label}/") } else { label.clone() };
			let file_kind = if is_dir {
				xeno_buffer_display::FileKind::Directory
			} else {
				xeno_buffer_display::FileKind::File
			};
			let file_meta = CompletionFileMeta::new(dir_path.join(&label), file_kind);

			scored.push((
				score as i32 + if is_dir { 40 } else { 0 },
				CompletionItem {
					label: insert_text.clone(),
					insert_text,
					detail: Some(if is_dir { "directory".into() } else { "file".into() }),
					filter_text: None,
					kind: CompletionKind::File,
					match_indices: if indices.is_empty() { None } else { Some(indices) },
					right: Some(if is_dir { "dir".into() } else { "file".into() }),
					file: Some(file_meta),
				},
			));
		}

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));

		scored.into_iter().map(|(_, item)| item).collect()
	}

	pub(super) fn build_option_key_items(query: &str) -> Vec<CompletionItem> {
		let query = query.trim();
		let mut scored: Vec<(i32, CompletionItem)> = OPTIONS
			.snapshot_guard()
			.iter_refs()
			.filter_map(|opt| {
				let name = opt.name_str();
				let mut best_score = i32::MIN;
				let mut match_indices: Option<Vec<usize>> = None;

				if let Some((score, _, indices)) = crate::completion::frizbee_match(query, name) {
					best_score = score as i32 + 200;
					if !indices.is_empty() {
						match_indices = Some(indices);
					}
				}

				for alias in opt.keys_resolved() {
					if let Some((score, _, _)) = crate::completion::frizbee_match(query, alias) {
						best_score = best_score.max(score as i32 + 80);
					}
				}

				if query.is_empty() {
					best_score = 0;
				}
				if !query.is_empty() && best_score == i32::MIN {
					return None;
				}

				Some((
					best_score,
					CompletionItem {
						label: name.to_string(),
						insert_text: name.to_string(),
						detail: Some("option".to_string()),
						filter_text: None,
						kind: CompletionKind::Command,
						match_indices,
						right: Some("opt".to_string()),
						file: None,
					},
				))
			})
			.collect();

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));
		scored.into_iter().map(|(_, item)| item).collect()
	}

	pub(super) fn build_option_value_items(query: &str, option_key: Option<&str>) -> Vec<CompletionItem> {
		let values: Vec<&str> = if let Some(key) = option_key
			&& let Some(opt) = xeno_registry::options::find(key)
		{
			match opt.value_type {
				OptionType::Bool => vec!["true", "false", "on", "off"],
				OptionType::Int => Vec::new(),
				OptionType::String => Vec::new(),
			}
		} else {
			vec!["true", "false", "on", "off"]
		};

		let query = query.trim();
		let mut scored = Vec::new();
		for value in values {
			if let Some((score, _, indices)) = crate::completion::frizbee_match(query, value) {
				scored.push((
					score as i32,
					CompletionItem {
						label: value.to_string(),
						insert_text: value.to_string(),
						detail: Some("value".to_string()),
						filter_text: None,
						kind: CompletionKind::Command,
						match_indices: if indices.is_empty() { None } else { Some(indices) },
						right: Some("value".to_string()),
						file: None,
					},
				));
			}
		}

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));
		scored.into_iter().map(|(_, item)| item).collect()
	}

	pub(super) fn build_items_for_token(
		&mut self,
		token: &TokenCtx,
		ctx: &dyn OverlayContext,
		session: &OverlaySession,
		usage: &crate::completion::CommandUsageSnapshot,
	) -> Vec<CompletionItem> {
		if token.token_index == 0 {
			let mut provider = FnPickerProvider::new(|query: &str| Self::build_command_items(query, usage));
			return provider.candidates(&token.query);
		}

		match Self::command_arg_completion(&token.cmd, token.token_index) {
			CommandArgCompletion::Theme => {
				let mut provider = FnPickerProvider::new(Self::build_theme_items);
				return provider.candidates(&token.query);
			}
			CommandArgCompletion::Snippet => {
				let query = token.query.trim_start();
				if !query.starts_with('@') {
					return Vec::new();
				}
				let mut provider = FnPickerProvider::new(Self::build_snippet_items);
				return provider.candidates(query);
			}
			CommandArgCompletion::FilePath => {
				let dir_part = token.path_dir.clone();
				let mut provider = FnPickerProvider::new(|query: &str| self.build_file_items(query, dir_part.as_deref(), ctx, session));
				return provider.candidates(&token.query);
			}
			CommandArgCompletion::OptionKey => {
				let mut provider = FnPickerProvider::new(Self::build_option_key_items);
				return provider.candidates(&token.query);
			}
			CommandArgCompletion::OptionValue => {
				let option_key = token.args.first().map(String::as_str);
				let mut provider = FnPickerProvider::new(|query: &str| Self::build_option_value_items(query, option_key));
				return provider.candidates(&token.query);
			}
			CommandArgCompletion::CommandName => {
				let mut provider = FnPickerProvider::new(|query: &str| Self::build_command_items(query, usage));
				return provider.candidates(&token.query);
			}
			CommandArgCompletion::None | CommandArgCompletion::Buffer | CommandArgCompletion::FreeText => {}
		}

		Vec::new()
	}
}
