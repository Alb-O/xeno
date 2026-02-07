use std::path::PathBuf;

use xeno_primitives::BoxFutureLocal;
use xeno_registry::actions::editor_ctx::OverlayRequest;
use xeno_registry::commands::{CommandEditorOps, CommandError};
use xeno_registry::notifications::Notification;
use xeno_registry::options::{OptionScope, find_by_kdl};
use xeno_registry::{
	EditorCapabilities, FileOpsAccess, HookContext, HookEventData, NotificationAccess, ThemeAccess,
	emit_sync_with as emit_hook_sync_with,
};

use crate::capabilities::provider::EditorCaps;

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

	fn set_option(&mut self, kdl_key: &str, value: &str) -> Result<(), CommandError> {
		let opt_value = super::parse_option_value(kdl_key, value)?;
		let _ = self
			.ed
			.state
			.config
			.global_options
			.set_by_kdl(kdl_key, opt_value);

		if let Some(def) = find_by_kdl(kdl_key) {
			emit_hook_sync_with(
				&HookContext::new(HookEventData::OptionChanged {
					key: def.kdl_key,
					scope: "global",
				}),
				&mut self.ed.state.hook_runtime,
			);
		}
		Ok(())
	}

	fn set_local_option(&mut self, kdl_key: &str, value: &str) -> Result<(), CommandError> {
		let def = find_by_kdl(kdl_key).ok_or_else(|| {
			use xeno_registry::options::parse;
			let suggestion = parse::suggest_option(kdl_key);
			CommandError::InvalidArgument(match suggestion {
				Some(s) => format!("unknown option '{kdl_key}'). Did you mean '{s}'?"),
				None => format!("unknown option '{kdl_key}'"),
			})
		})?;

		if def.scope == OptionScope::Global {
			return Err(CommandError::InvalidArgument(format!(
				"'{kdl_key}' is a global option, use :set instead of :setlocal"
			)));
		}

		let opt_value = super::parse_option_value(kdl_key, value)?;
		let _ = self
			.ed
			.buffer_mut()
			.local_options
			.set_by_kdl(kdl_key, opt_value);

		emit_hook_sync_with(
			&HookContext::new(HookEventData::OptionChanged {
				key: def.kdl_key,
				scope: "buffer",
			}),
			&mut self.ed.state.hook_runtime,
		);
		Ok(())
	}

	fn open_info_popup(&mut self, content: &str, _file_type: Option<&str>) {
		self.ed
			.state
			.effects
			.overlay_request(OverlayRequest::ShowInfoPopup {
				title: None,
				body: content.to_string(),
			});
	}

	fn close_all_info_popups(&mut self) {
		// TODO: Add CloseInfoPopups to OverlayRequest if needed
	}

	fn goto_file(
		&mut self,
		path: PathBuf,
		line: usize,
		column: usize,
	) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			use crate::impls::Location;
			self.ed
				.goto_location(&Location::new(path, line, column))
				.await
				.map_err(|e| CommandError::Io(e.to_string()))?;
			Ok(())
		})
	}
}
