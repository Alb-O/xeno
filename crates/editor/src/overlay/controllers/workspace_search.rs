use std::future::Future;
use std::pin::Pin;

use termina::event::{KeyCode, KeyEvent};
use xeno_broker_proto::types::KnowledgeHit;
#[cfg(feature = "lsp")]
use xeno_broker_proto::types::{RequestPayload, ResponsePayload};
#[cfg(feature = "lsp")]
use xeno_primitives::Selection;
use xeno_registry::notifications::keys;
use xeno_registry::options::OptionValue;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::buffer::ViewId;
use crate::impls::Editor;
use crate::overlay::{
	CloseReason, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy, WindowRole,
	WindowSpec,
};
use crate::window::{FloatingStyle, GutterSelector};

#[cfg(feature = "lsp")]
const SEARCH_LIMIT: u32 = 50;

pub struct WorkspaceSearchOverlay {
	results: Vec<KnowledgeHit>,
	selected: usize,
	last_query: Option<String>,
	list_buffer: Option<ViewId>,
}

impl Default for WorkspaceSearchOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceSearchOverlay {
	pub fn new() -> Self {
		Self {
			results: Vec::new(),
			selected: 0,
			last_query: None,
			list_buffer: None,
		}
	}

	fn list_buffer_id(&self, session: &OverlaySession) -> Option<ViewId> {
		self.list_buffer.or_else(|| {
			session
				.buffers
				.iter()
				.copied()
				.find(|id| *id != session.input)
		})
	}

	fn set_list_content(&self, ed: &mut Editor, session: &OverlaySession, content: String) {
		let Some(buffer_id) = self.list_buffer_id(session) else {
			return;
		};
		if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(buffer_id) {
			buffer.reset_content(content);
		}
	}

	fn render_results(&self, ed: &mut Editor, session: &OverlaySession) {
		if self.results.is_empty() {
			self.set_list_content(ed, session, "No results".to_string());
			return;
		}

		let mut lines = Vec::with_capacity(self.results.len());
		for (idx, hit) in self.results.iter().enumerate() {
			let marker = if idx == self.selected { '>' } else { ' ' };
			let preview = hit.preview.replace('\n', " ");
			lines.push(format!(
				"{marker} {score:>6.2} {uri} [{start}-{end}] {preview}",
				score = hit.score,
				uri = hit.uri,
				start = hit.start_char,
				end = hit.end_char,
				preview = preview
			));
		}

		self.set_list_content(ed, session, lines.join("\n"));
	}
}

impl OverlayController for WorkspaceSearchOverlay {
	fn name(&self) -> &'static str {
		"WorkspaceSearch"
	}

	fn ui_spec(&self, _ed: &Editor) -> OverlayUiSpec {
		let mut buffer_options = std::collections::HashMap::new();
		buffer_options.insert("cursorline".into(), OptionValue::Bool(false));

		OverlayUiSpec {
			title: Some("Workspace Search".into()),
			gutter: GutterSelector::Prompt('/'),
			rect: RectPolicy::TopCenter {
				width_percent: 70,
				max_width: 100,
				min_width: 50,
				y_frac: (1, 6),
				height: 3,
			},
			style: crate::overlay::prompt_style("Workspace Search"),
			windows: vec![WindowSpec {
				role: WindowRole::List,
				rect: RectPolicy::Below(WindowRole::Input, 0, 15),
				style: FloatingStyle {
					border: true,
					border_type: BorderType::Rounded,
					padding: Padding::ZERO,
					shadow: false,
					title: None,
				},
				buffer_options,
				dismiss_on_blur: false,
				sticky: false,
				gutter: GutterSelector::Hidden,
			}],
		}
	}

	fn on_open(&mut self, ed: &mut Editor, session: &mut OverlaySession) {
		self.list_buffer = session
			.buffers
			.iter()
			.copied()
			.find(|id| *id != session.input);
		self.set_list_content(ed, session, "Type a query and press Enter".to_string());
	}

	fn on_input_changed(&mut self, ed: &mut Editor, session: &mut OverlaySession, text: &str) {
		let trimmed = text.trim();
		if trimmed.is_empty() {
			self.results.clear();
			self.selected = 0;
			self.last_query = None;
			self.set_list_content(ed, session, "Type a query and press Enter".to_string());
			return;
		}

		if self
			.last_query
			.as_deref()
			.is_some_and(|last| last == trimmed)
		{
			return;
		}

		self.results.clear();
		self.selected = 0;
		self.last_query = None;
		self.set_list_content(ed, session, "Press Enter to search".to_string());
	}

	fn on_key(&mut self, ed: &mut Editor, session: &mut OverlaySession, key: KeyEvent) -> bool {
		if self.results.is_empty() {
			return false;
		}

		match key.code {
			KeyCode::Up => {
				if self.selected > 0 {
					self.selected -= 1;
					self.render_results(ed, session);
				}
				true
			}
			KeyCode::Down => {
				if self.selected + 1 < self.results.len() {
					self.selected += 1;
					self.render_results(ed, session);
				}
				true
			}
			_ => false,
		}
	}

	fn on_commit<'a>(
		&'a mut self,
		ed: &'a mut Editor,
		session: &'a mut OverlaySession,
	) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let query = session
			.input_text(ed)
			.trim_end_matches('\n')
			.trim()
			.to_string();

		Box::pin(async move {
			if query.is_empty() {
				return;
			}

			let should_search = self
				.last_query
				.as_deref()
				.is_none_or(|last| last != query)
				|| self.results.is_empty();

			if should_search {
				#[cfg(feature = "lsp")]
				{
					let broker = ed.state.lsp.broker_transport();
					match broker
						.buffer_sync_request(RequestPayload::KnowledgeSearch {
							query: query.clone(),
							limit: SEARCH_LIMIT,
						})
						.await
					{
						Ok(ResponsePayload::KnowledgeSearchResults { hits }) => {
							self.results = hits;
							self.selected = 0;
							self.last_query = Some(query.clone());
							self.render_results(ed, session);
						}
						Ok(_) => {
							self.results.clear();
							self.selected = 0;
							self.set_list_content(
								ed,
								session,
								"Unexpected response from broker".to_string(),
							);
							ed.notify(keys::error("Unexpected broker response"));
							return;
						}
						Err(err) => {
							self.results.clear();
							self.selected = 0;
							self.set_list_content(ed, session, "Search failed".to_string());
							ed.notify(keys::error(err.to_string()));
							return;
						}
					}
				}
				#[cfg(not(feature = "lsp"))]
				{
					let _ = &query;
					ed.notify(keys::warn("LSP not enabled"));
					return;
				}
			}

			if self.results.is_empty() {
				ed.notify(keys::info("No results"));
				return;
			}

			let hit = self.results.get(self.selected).cloned();
			let Some(hit) = hit else { return };

			open_hit(ed, &hit).await;
		})
	}

	fn on_close(&mut self, _ed: &mut Editor, _session: &mut OverlaySession, _reason: CloseReason) {}
}

async fn open_hit(ed: &mut Editor, hit: &KnowledgeHit) {
	#[cfg(feature = "lsp")]
	{
		let Ok(uri) = hit.uri.parse::<xeno_lsp::lsp_types::Uri>() else {
			ed.notify(keys::error("Invalid URI"));
			return;
		};
		let Some(path) = xeno_lsp::path_from_uri(&uri) else {
			ed.notify(keys::error("Unsupported URI"));
			return;
		};

		let buffer_id = if let Some(existing) = ed.state.core.buffers.find_by_path(&path) {
			existing
		} else {
			match ed.open_file(path).await {
				Ok(id) => id,
				Err(err) => {
					ed.notify(keys::error(err.to_string()));
					return;
				}
			}
		};

		ed.focus_buffer(buffer_id);

		if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(buffer_id) {
			let max = buffer.with_doc(|doc| doc.content().len_chars());
			let start = (hit.start_char as usize).min(max);
			let end = (hit.end_char as usize).min(max).max(start);
			buffer.set_cursor_and_selection(start, Selection::single(start, end));
		}

		ed.reveal_cursor_in_view(buffer_id);
	}
	#[cfg(not(feature = "lsp"))]
	{
		let _ = hit;
		ed.notify(keys::warn("LSP not enabled"));
	}
}
