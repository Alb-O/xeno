use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use parking_lot::Mutex;
use tome_cabi_types::{
	TomeBool, TomeChatRole, TomeOwnedStr, TomePanelId, TomePermissionOptionV1,
	TomePermissionRequestV1, TomePluginEventKind, TomePluginEventV1,
};

use crate::state::SharedState;
use crate::utils::{string_to_tome_owned, strip_ansi_and_controls};

pub struct SendEvent(pub TomePluginEventV1);
unsafe impl Send for SendEvent {}
unsafe impl Sync for SendEvent {}

pub fn enqueue_line(
	events: &Arc<Mutex<VecDeque<SendEvent>>>,
	panel_id: &Arc<Mutex<Option<TomePanelId>>>,
	msg: String,
) {
	let msg = strip_ansi_and_controls(&msg);

	let mut events = events.lock();
	if let Some(pid) = *panel_id.lock() {
		events.push_back(SendEvent(TomePluginEventV1 {
			kind: TomePluginEventKind::PanelAppend,
			panel_id: pid,
			role: TomeChatRole::System,
			text: string_to_tome_owned(msg),
			bool_val: TomeBool(0),
			permission_request_id: 0,
			permission_request: std::ptr::null_mut(),
		}));
	} else {
		events.push_back(SendEvent(TomePluginEventV1 {
			kind: TomePluginEventKind::ShowMessage,
			panel_id: 0,
			role: TomeChatRole::System,
			text: string_to_tome_owned(msg),
			bool_val: TomeBool(0),
			permission_request_id: 0,
			permission_request: std::ptr::null_mut(),
		}));
	}
}

pub fn enqueue_panel_append(
	events: &Arc<Mutex<VecDeque<SendEvent>>>,
	panel_id: TomePanelId,
	role: TomeChatRole,
	text: String,
) {
	let mut events = events.lock();
	events.push_back(SendEvent(TomePluginEventV1 {
		kind: TomePluginEventKind::PanelAppend,
		panel_id,
		role,
		text: string_to_tome_owned(text),
		bool_val: TomeBool(0),
		permission_request_id: 0,
		permission_request: std::ptr::null_mut(),
	}));
}

pub async fn request_permission(
	state: &SharedState,
	prompt: &str,
) -> agent_client_protocol::Result<bool> {
	let (tx, rx) = tokio::sync::oneshot::channel();
	let id = state.next_permission_id.fetch_add(1, Ordering::SeqCst);

	{
		let mut pending = state.pending_permissions.lock();
		pending.insert(id, tx);
	}

	let pid = state.panel_id.lock().unwrap_or(0);
	let prompt_tome = string_to_tome_owned(prompt.to_string());

	let options = vec![
		TomePermissionOptionV1 {
			option_id: string_to_tome_owned("allow".to_string()),
			label: string_to_tome_owned("Allow".to_string()),
		},
		TomePermissionOptionV1 {
			option_id: string_to_tome_owned("deny".to_string()),
			label: string_to_tome_owned("Deny".to_string()),
		},
	];
	let options_len = options.len();
	let options_ptr = Box::into_raw(options.into_boxed_slice()) as *mut TomePermissionOptionV1;

	let req = Box::new(TomePermissionRequestV1 {
		prompt: prompt_tome,
		options: options_ptr,
		options_len,
	});
	let req_ptr = Box::into_raw(req);

	{
		let mut events_guard = state.events.lock();
		events_guard.push_back(SendEvent(TomePluginEventV1 {
			kind: TomePluginEventKind::RequestPermission,
			panel_id: pid,
			role: TomeChatRole::System,
			text: TomeOwnedStr {
				ptr: std::ptr::null_mut(),
				len: 0,
			},
			bool_val: TomeBool(0),
			permission_request_id: id,
			permission_request: req_ptr,
		}));
	}

	match rx.await {
		Ok(decision) => Ok(decision == "allow"),
		Err(_) => Err(agent_client_protocol::Error::new(
			-32603,
			"Internal error: permission channel closed".to_string(),
		)),
	}
}
