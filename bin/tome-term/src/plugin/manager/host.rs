use std::cell::RefCell;
use std::path::PathBuf;

use tome_cabi_types::{
	TOME_C_ABI_VERSION_V2, TomeBool, TomeChatRole, TomeCommandSpecV1, TomeHostPanelApiV1,
	TomeHostV2, TomeMessageKind, TomeOwnedStr, TomePanelId, TomePanelKind, TomeStatus, TomeStr,
};

use crate::editor::Editor;
use crate::plugin::manager::PluginManager;
use crate::plugin::manager::context::{ACTIVE_EDITOR, ACTIVE_MANAGER};
use crate::plugin::panels::ChatItem;

pub static HOST_V2: TomeHostV2 = TomeHostV2 {
	struct_size: std::mem::size_of::<TomeHostV2>(),
	abi_version: TOME_C_ABI_VERSION_V2,
	log: Some(host_log),
	panel: TomeHostPanelApiV1 {
		create: host_panel_create,
		set_open: host_panel_set_open,
		set_focused: host_panel_set_focused,
		append_transcript: host_panel_append_transcript,
		request_redraw: host_request_redraw,
	},
	show_message: host_show_message,
	insert_text: host_insert_text,
	register_command: Some(host_register_command),
	get_current_path: Some(host_get_current_path),
	free_str: Some(host_free_str),
	fs_read_text: Some(host_fs_read_text),
	fs_write_text: Some(host_fs_write_text),
};

pub fn tome_str_to_str(ts: &TomeStr) -> &str {
	if ts.ptr.is_null() {
		return "";
	}
	unsafe {
		let slice = std::slice::from_raw_parts(ts.ptr, ts.len);
		std::str::from_utf8(slice).unwrap_or("<invalid utf-8>")
	}
}

pub fn tome_owned_to_string(tos: TomeOwnedStr) -> Option<String> {
	if tos.ptr.is_null() {
		return None;
	}
	unsafe {
		let slice = std::slice::from_raw_parts(tos.ptr, tos.len);
		Some(String::from_utf8_lossy(slice).into_owned())
	}
}

pub fn string_to_tome_owned(s: String) -> TomeOwnedStr {
	let bytes = s.into_bytes().into_boxed_slice();
	let len = bytes.len();
	let ptr = Box::into_raw(bytes) as *mut u8;
	TomeOwnedStr { ptr, len }
}

pub(crate) extern "C" fn host_log(msg: TomeStr) {
	let s = tome_str_to_str(&msg).to_string();
	ACTIVE_MANAGER.with(|mgr_ctx: &RefCell<Option<*mut PluginManager>>| {
		ACTIVE_EDITOR.with(|ed_ctx: &RefCell<Option<*mut Editor>>| {
			if let (Some(mgr_ptr), Some(_ed_ptr)) = (*mgr_ctx.borrow(), *ed_ctx.borrow()) {
				let mgr = unsafe { &mut *mgr_ptr };
				if let Some(id) = &mgr.current_plugin_id {
					mgr.logs.entry(id.clone()).or_default().push(s);
				}
			}
		});
	});
}

pub(crate) extern "C" fn host_panel_create(kind: TomePanelKind, title: TomeStr) -> TomePanelId {
	ACTIVE_MANAGER.with(|ctx: &RefCell<Option<*mut PluginManager>>| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			let id = mgr.next_panel_id;
			mgr.next_panel_id += 1;

			if let Some(plugin_id) = &mgr.current_plugin_id {
				mgr.panel_owners.insert(id, plugin_id.clone());
			}

			let title_str = tome_str_to_str(&title).to_string();
			match kind {
				TomePanelKind::Chat => {
					mgr.panels.insert(
						id,
						crate::plugin::panels::ChatPanelState::new(title_str.clone()),
					);

					let ui_id = crate::ui::panels::chat::chat_panel_ui_id(id);
					mgr.panel_ui_ids.insert(id, ui_id.clone());

					ACTIVE_EDITOR.with(|ed_ctx: &RefCell<Option<*mut Editor>>| {
						if let Some(ed_ptr) = *ed_ctx.borrow() {
							let ed = unsafe { &mut *ed_ptr };
							ed.ui.register_panel(Box::new(
								crate::ui::panels::chat::PluginChatPanel::new(id, title_str),
							));
							ed.request_redraw();
						}
					});
				}
			}
			id
		} else {
			0
		}
	})
}

pub(crate) extern "C" fn host_panel_set_open(id: TomePanelId, open: TomeBool) {
	ACTIVE_MANAGER.with(|ctx: &RefCell<Option<*mut PluginManager>>| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			if mgr.panel_owners.get(&id) == mgr.current_plugin_id.as_ref()
				&& mgr.panels.contains_key(&id)
				&& let Some(ui_id) = mgr.panel_ui_ids.get(&id).cloned()
			{
				ACTIVE_EDITOR.with(|ed_ctx: &RefCell<Option<*mut Editor>>| {
					if let Some(ed_ptr) = *ed_ctx.borrow() {
						let ed = unsafe { &mut *ed_ptr };
						ed.ui.set_open(&ui_id, open.0 != 0);
						ed.request_redraw();
					}
				});
			}
		}
	})
}

pub(crate) extern "C" fn host_panel_set_focused(id: TomePanelId, focused: TomeBool) {
	ACTIVE_MANAGER.with(|ctx: &RefCell<Option<*mut PluginManager>>| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			if mgr.panel_owners.get(&id) == mgr.current_plugin_id.as_ref()
				&& mgr.panels.contains_key(&id)
				&& let Some(ui_id) = mgr.panel_ui_ids.get(&id).cloned()
			{
				ACTIVE_EDITOR.with(|ed_ctx: &RefCell<Option<*mut Editor>>| {
					if let Some(ed_ptr) = *ed_ctx.borrow() {
						let ed = unsafe { &mut *ed_ptr };
						if focused.0 != 0 {
							ed.ui.set_open(&ui_id, true);
							ed.ui.apply_requests(vec![crate::ui::UiRequest::Focus(
								crate::ui::FocusTarget::panel(ui_id.clone()),
							)]);
						} else if ed.ui.is_panel_focused(&ui_id) {
							ed.ui.apply_requests(vec![crate::ui::UiRequest::Focus(
								crate::ui::FocusTarget::editor(),
							)]);
						}
						ed.request_redraw();
					}
				});
			}
		}
	})
}

pub(crate) extern "C" fn host_panel_append_transcript(
	id: TomePanelId,
	role: TomeChatRole,
	text: TomeStr,
) {
	ACTIVE_MANAGER.with(|ctx: &RefCell<Option<*mut PluginManager>>| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			if mgr.panel_owners.get(&id) == mgr.current_plugin_id.as_ref()
				&& let Some(panel) = mgr.panels.get_mut(&id)
			{
				panel.transcript.push(ChatItem {
					_role: role,
					_text: tome_str_to_str(&text).to_string(),
				});
				ACTIVE_EDITOR.with(|ed_ctx: &RefCell<Option<*mut Editor>>| {
					if let Some(ed_ptr) = *ed_ctx.borrow() {
						let ed = unsafe { &mut *ed_ptr };
						ed.request_redraw();
					}
				});
			}
		}
	})
}

pub(crate) extern "C" fn host_request_redraw() {
	ACTIVE_EDITOR.with(|ctx: &RefCell<Option<*mut Editor>>| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			ed.request_redraw();
		}
	})
}

pub(crate) extern "C" fn host_show_message(kind: TomeMessageKind, msg: TomeStr) {
	let s = tome_str_to_str(&msg);
	ACTIVE_EDITOR.with(|ctx: &RefCell<Option<*mut Editor>>| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			match kind {
				TomeMessageKind::Info => ed.show_message(s),
				TomeMessageKind::Error => ed.show_error(s),
			}
		}
	})
}

pub(crate) extern "C" fn host_insert_text(text: TomeStr) {
	let s = tome_str_to_str(&text);
	ACTIVE_EDITOR.with(|ctx: &RefCell<Option<*mut Editor>>| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			ed.insert_text(s);
		}
	})
}

pub(crate) extern "C" fn host_get_current_path(out: *mut TomeOwnedStr) -> TomeStatus {
	ACTIVE_EDITOR.with(|ctx: &RefCell<Option<*mut Editor>>| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &*ed_ptr };
			let path = ed.path.as_ref();
			if let Some(path) = path {
				let s: String = path.to_string_lossy().to_string();
				unsafe { *out = string_to_tome_owned(s) };
				TomeStatus::Ok
			} else {
				TomeStatus::Failed
			}
		} else {
			TomeStatus::AccessDenied
		}
	})
}

pub(crate) extern "C" fn host_free_str(s: TomeOwnedStr) {
	if s.ptr.is_null() {
		return;
	}
	unsafe {
		let slice = std::ptr::slice_from_raw_parts_mut(s.ptr, s.len);
		drop(Box::from_raw(slice));
	}
}

pub(crate) extern "C" fn host_fs_read_text(path: TomeStr, out: *mut TomeOwnedStr) -> TomeStatus {
	ACTIVE_EDITOR.with(|ctx: &RefCell<Option<*mut Editor>>| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			let path_str = tome_str_to_str(&path);
			let path_buf = PathBuf::from(path_str);

			let allowed = if let Some(current_path) = ed.path.as_ref() {
				if let Some(parent) = current_path.parent() {
					path_buf.starts_with(parent)
				} else {
					false
				}
			} else {
				false
			};

			if !allowed {
				ed.show_error(format!(
					"Plugin tried to read restricted path: {}",
					path_str
				));
				return TomeStatus::AccessDenied;
			}

			match std::fs::read_to_string(path_buf) {
				Ok(content) => {
					unsafe { *out = string_to_tome_owned(content) };
					TomeStatus::Ok
				}
				Err(_) => TomeStatus::Failed,
			}
		} else {
			TomeStatus::AccessDenied
		}
	})
}

pub(crate) extern "C" fn host_fs_write_text(path: TomeStr, content: TomeStr) -> TomeStatus {
	ACTIVE_EDITOR.with(|ctx: &RefCell<Option<*mut Editor>>| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			let path_str = tome_str_to_str(&path);
			let path_buf = PathBuf::from(path_str);
			let content_str = tome_str_to_str(&content);

			let allowed = if let Some(current_path) = ed.path.as_ref() {
				if let Some(parent) = current_path.parent() {
					path_buf.starts_with(parent)
				} else {
					false
				}
			} else {
				false
			};

			if !allowed {
				ed.show_error(format!(
					"Plugin tried to write to restricted path: {}",
					path_str
				));
				return TomeStatus::AccessDenied;
			}

			match std::fs::write(path_buf, content_str) {
				Ok(_) => TomeStatus::Ok,
				Err(_) => TomeStatus::Failed,
			}
		} else {
			TomeStatus::AccessDenied
		}
	})
}

pub(crate) extern "C" fn host_register_command(spec: TomeCommandSpecV1) {
	ACTIVE_MANAGER.with(|ctx: &RefCell<Option<*mut PluginManager>>| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			mgr.register_command(spec);
		}
	})
}
