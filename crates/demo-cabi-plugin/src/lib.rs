#![allow(non_camel_case_types)]
//! Minimal example C-ABI Tome plugin using the shared tome-cabi-types crate.

use tome_cabi_types::{
	TOME_C_ABI_VERSION_V2, TomeBool, TomeChatRole, TomeCommandContextV1, TomeCommandSpecV1,
	TomeGuestV2, TomeHostV2, TomePanelKind, TomeStatus, TomeStr, TomeStrArray,
};

/// # Safety
/// This function is called by the host when the plugin is loaded.
/// It must be called with valid pointers to `TomeHostV2` and `TomeGuestV2`.
#[unsafe(no_mangle)]
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
			abi_version: TOME_C_ABI_VERSION_V2,
			namespace: tome_str("demo"),
			name: tome_str("Demo Plugin"),
			version: tome_str("0.1.0"),
			init: Some(plugin_init),
			shutdown: None,
			poll_event: None,
			free_str: None,
			on_panel_submit: None,
			on_permission_decision: None,
			free_permission_request: None,
		};
	}

	TomeStatus::Ok
}

extern "C" fn plugin_init(host: *const TomeHostV2) -> TomeStatus {
	let host = unsafe { &*host };

	let cmd = TomeCommandSpecV1 {
		name: tome_str("hello"),
		aliases: TomeStrArray {
			ptr: std::ptr::null(),
			len: 0,
		},
		description: tome_str("Say hello from demo plugin"),
		handler: Some(hello_handler),
		user_data: std::ptr::null_mut(),
	};

	if let Some(reg) = host.register_command {
		reg(cmd);
	}

	TomeStatus::Ok
}

extern "C" fn hello_handler(ctx: *mut TomeCommandContextV1) -> TomeStatus {
	let host = unsafe { &*(*ctx).host };

	let panel_id = (host.panel.create)(TomePanelKind::Chat, tome_str("Demo Panel"));
	(host.panel.set_open)(panel_id, TomeBool(1));
	(host.panel.set_focused)(panel_id, TomeBool(1));
	(host.panel.append_transcript)(
		panel_id,
		TomeChatRole::Assistant,
		tome_str("Hello! This is the V2 Demo Plugin speaking from its own panel."),
	);

	TomeStatus::Ok
}

fn tome_str(s: &'static str) -> TomeStr {
	TomeStr {
		ptr: s.as_ptr(),
		len: s.len(),
	}
}
