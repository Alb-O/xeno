mod backend;
mod commands;
mod events;
mod handler;
mod state;
mod utils;

use std::cell::RefCell;
use std::path::PathBuf;
use std::thread;

use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, Sender};
use tokio::task::LocalSet;
use tome_cabi_types::{
	TOME_C_ABI_VERSION_V2, TomeBool, TomeCommandSpecV1, TomeGuestV2, TomeHostV2, TomeOwnedStr,
	TomePanelId, TomePermissionRequestId, TomePermissionRequestV1, TomePluginEventV1, TomeStatus,
	TomeStr, TomeStrArray,
};

use crate::backend::AcpBackend;
use crate::commands::{
	command_cancel, command_insert_last, command_start, command_stop, command_toggle,
};
use crate::state::{HostHandle, SharedState};
use crate::utils::{plugin_free_str, tome_str, tome_str_to_string};

thread_local! {
	static PLUGIN: RefCell<Option<AcpPlugin>> = const { RefCell::new(None) };
}

struct AcpPlugin {
	#[allow(dead_code)]
	host: *const TomeHostV2,
	cmd_tx: Sender<AgentCommand>,
	state: SharedState,
}

pub enum AgentCommand {
	Start { cwd: PathBuf },
	Stop,
	Prompt { content: String },
	Cancel,
}

#[unsafe(no_mangle)]
/// # Safety
/// - `host` must be a valid pointer to a live `TomeHostV2` provided by the Tome host.
/// - `out_guest` must be a valid pointer to writable storage for a `TomeGuestV2`.
/// - Both pointers must remain valid for the duration of this call.
pub unsafe extern "C" fn tome_plugin_entry_v2(
	host: *const TomeHostV2,
	out_guest: *mut TomeGuestV2,
) -> TomeStatus {
	if host.is_null() || out_guest.is_null() {
		return TomeStatus::Failed;
	}

	let host_ref = unsafe { &*host };
	if host_ref.abi_version != TOME_C_ABI_VERSION_V2 {
		return TomeStatus::Incompatible;
	}

	unsafe {
		*out_guest = TomeGuestV2 {
			struct_size: std::mem::size_of::<TomeGuestV2>(),
			abi_version: TOME_C_ABI_VERSION_V2,
			namespace: tome_str("acp"),
			name: tome_str("ACP Agent"),
			version: tome_str("0.1.0"),
			init: Some(plugin_init),
			shutdown: Some(plugin_shutdown),
			poll_event: Some(plugin_poll_event),
			free_str: Some(plugin_free_str_ffi),
			on_panel_submit: Some(plugin_on_panel_submit),
			on_permission_decision: Some(plugin_on_permission_decision),
			free_permission_request: Some(plugin_free_permission_request),
		};
	}

	TomeStatus::Ok
}

extern "C" fn plugin_init(host: *const TomeHostV2) -> TomeStatus {
	let (cmd_tx, cmd_rx) = mpsc::channel(100);
	let state = SharedState::new();

	let state_clone = state.clone();
	let host_handle = HostHandle::new(host);

	thread::spawn(move || {
		let rt = Runtime::new().unwrap();
		let local = LocalSet::new();

		local.block_on(&rt, async {
			AcpBackend::new(host_handle, cmd_rx, state_clone)
				.run()
				.await;
		});
	});

	PLUGIN.with(|ctx| {
		*ctx.borrow_mut() = Some(AcpPlugin {
			host,
			cmd_tx,
			state,
		});
	});

	register_commands(host);

	TomeStatus::Ok
}

fn register_commands(host: *const TomeHostV2) {
	let host_ref = unsafe { &*host };
	let Some(reg) = host_ref.register_command else {
		return;
	};

	let commands = [
		("start", "Start the ACP agent", command_start as _),
		("stop", "Stop the ACP agent", command_stop as _),
		("toggle", "Toggle the ACP agent panel", command_toggle as _),
		(
			"insert_last",
			"Insert last response",
			command_insert_last as _,
		),
		(
			"cancel",
			"Cancel the in-flight request",
			command_cancel as _,
		),
	];

	for (name, desc, handler) in commands {
		reg(TomeCommandSpecV1 {
			name: tome_str(name),
			aliases: TomeStrArray {
				ptr: std::ptr::null(),
				len: 0,
			},
			description: tome_str(desc),
			handler: Some(handler),
			user_data: std::ptr::null_mut(),
		});
	}
}

extern "C" fn plugin_shutdown() {
	PLUGIN.with(|ctx| {
		if let Some(plugin) = ctx.borrow_mut().take() {
			let _ = plugin.cmd_tx.try_send(AgentCommand::Stop);
		}
	});
}

extern "C" fn plugin_poll_event(out: *mut TomePluginEventV1) -> TomeBool {
	if out.is_null() {
		return TomeBool(0);
	}
	PLUGIN.with(|ctx| {
		if let Some(plugin) = ctx.borrow().as_ref() {
			let mut events = plugin.state.events.lock();
			if let Some(event) = events.pop_front() {
				unsafe { *out = event.0 };
				return TomeBool(1);
			}
		}
		TomeBool(0)
	})
}

extern "C" fn plugin_free_str_ffi(s: TomeOwnedStr) {
	plugin_free_str(s);
}

extern "C" fn plugin_free_permission_request(req: *mut TomePermissionRequestV1) {
	if req.is_null() {
		return;
	}

	unsafe {
		let req = Box::from_raw(req);
		plugin_free_str(req.prompt);
		if !req.options.is_null() {
			let slice = std::slice::from_raw_parts_mut(req.options, req.options_len);
			for opt in slice.iter() {
				plugin_free_str(opt.option_id);
				plugin_free_str(opt.label);
			}
			drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
				req.options,
				req.options_len,
			)));
		}
	}
}

extern "C" fn plugin_on_panel_submit(id: TomePanelId, text: TomeStr) {
	PLUGIN.with(|ctx| {
		if let Some(plugin) = ctx.borrow().as_ref() {
			let pid = plugin.state.panel_id.lock();
			if Some(id) == *pid {
				let s = tome_str_to_string(text);
				let _ = plugin.cmd_tx.try_send(AgentCommand::Prompt { content: s });
			}
		}
	});
}

extern "C" fn plugin_on_permission_decision(id: TomePermissionRequestId, option_id: TomeStr) {
	PLUGIN.with(|ctx| {
		if let Some(plugin) = ctx.borrow().as_ref() {
			let mut pending = plugin.state.pending_permissions.lock();
			if let Some(tx) = pending.remove(&id) {
				let s = tome_str_to_string(option_id);
				let _ = tx.send(s);
			}
		}
	});
}
