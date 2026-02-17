use std::path::PathBuf;

use xeno_primitives::BoxFutureLocal;
use xeno_registry::HookEventData;
use xeno_registry::actions::editor_ctx::OverlayRequest;
use xeno_registry::actions::{EditorCapabilities, FileOpsAccess, NotificationAccess, ThemeAccess};
use xeno_registry::commands::{CommandEditorOps, CommandError};
use xeno_registry::hooks::{HookContext, emit_sync_with as emit_hook_sync_with};
use xeno_registry::notifications::Notification;
use xeno_registry::options::{OptionScope, find};

use crate::capabilities::provider::EditorCaps;
use crate::runtime::work_queue::RuntimeWorkSource;

impl CommandEditorOps for EditorCaps<'_> {
	fn emit(&mut self, notification: Notification) {
		NotificationAccess::emit(self, notification);
	}

	fn clear_notifications(&mut self) {
		NotificationAccess::clear_notifications(self);
	}

	fn is_modified(&self) -> bool {
		FileOpsAccess::is_modified(self)
	}

	fn is_readonly(&self) -> bool {
		EditorCapabilities::is_readonly(self)
	}

	fn set_readonly(&mut self, readonly: bool) {
		self.ed.buffer_mut().set_readonly(readonly);
	}

	fn save(&mut self) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		FileOpsAccess::save(self)
	}

	fn save_as(&mut self, path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		FileOpsAccess::save_as(self, path)
	}

	fn set_theme(&mut self, name: &str) -> Result<(), CommandError> {
		ThemeAccess::set_theme(self, name)
	}

	fn set_option(&mut self, key: &str, value: &str) -> Result<(), CommandError> {
		let opt_value = super::parse_option_value(key, value)?;
		let _ = self.ed.state.config.global_options.set_by_key(&xeno_registry::db::OPTIONS, key, opt_value);

		if let Some(def) = find(key) {
			let resolved_key = def.name_str();
			emit_hook_sync_with(
				&HookContext::new(HookEventData::OptionChanged {
					key: resolved_key,
					scope: "global",
				}),
				&mut self.ed.state.work_scheduler,
			);
		}
		Ok(())
	}

	fn set_local_option(&mut self, key: &str, value: &str) -> Result<(), CommandError> {
		let def = find(key).ok_or_else(|| {
			use xeno_registry::options::parse;
			let suggestion = parse::suggest_option(key);
			CommandError::InvalidArgument(match suggestion {
				Some(s) => format!("unknown option '{key}'). Did you mean '{s}'?"),
				None => format!("unknown option '{key}'"),
			})
		})?;

		if def.scope == OptionScope::Global {
			return Err(CommandError::InvalidArgument(format!(
				"'{key}' is a global option, use :set instead of :setlocal"
			)));
		}

		let opt_value = super::parse_option_value(key, value)?;
		let _ = self.ed.buffer_mut().local_options.set_by_key(&xeno_registry::db::OPTIONS, key, opt_value);

		let resolved_key = def.name_str();
		emit_hook_sync_with(
			&HookContext::new(HookEventData::OptionChanged {
				key: resolved_key,
				scope: "buffer",
			}),
			&mut self.ed.state.work_scheduler,
		);
		Ok(())
	}

	fn open_info_popup(&mut self, content: &str, _file_type: Option<&str>) {
		self.ed.state.effects.overlay_request(OverlayRequest::ShowInfoPopup {
			title: None,
			body: content.to_string(),
		});
	}

	fn close_all_info_popups(&mut self) {
		// TODO: Add CloseInfoPopups to OverlayRequest if needed
	}

	fn insert_snippet_body(&mut self, body: &str) -> bool {
		self.ed.insert_snippet_body(body)
	}

	fn goto_file(&mut self, path: PathBuf, line: usize, column: usize) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			use crate::impls::Location;
			let target = crate::paths::fast_abs(&path);

			let current = self.ed.buffer().path().map(|current| crate::paths::fast_abs(&current));
			let switching_files = current.as_ref().map(|current| current != &target).unwrap_or(true);

			if switching_files && FileOpsAccess::is_modified(self) {
				return Err(CommandError::Other("No write since last change".to_string()));
			}

			self.ed
				.goto_location(&Location::new(target, line, column))
				.await
				.map_err(|e| CommandError::Io(e.to_string()))?;
			Ok(())
		})
	}

	fn queue_invocation(&mut self, request: xeno_registry::actions::DeferredInvocationRequest) {
		self.ed.enqueue_runtime_invocation_request(request, RuntimeWorkSource::CommandOps);
	}
}
