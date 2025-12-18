use std::path::PathBuf;

use tome_cabi_types::{
	TomeBool, TomeCommandContextV1, TomeOwnedStr, TomePanelKind, TomeStatus, TomeStr,
};

use crate::utils::{tome_owned_to_string, tome_str};
use crate::{AgentCommand, PLUGIN};

pub extern "C" fn command_start(ctx: *mut TomeCommandContextV1) -> TomeStatus {
	PLUGIN.with(|p_ctx| {
		if let Some(plugin) = p_ctx.borrow().as_ref() {
			let host = unsafe { &*(*ctx).host };
			let mut cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

			if let (Some(get_path), Some(free_str)) = (host.get_current_path, host.free_str) {
				let mut owned_str = unsafe { std::mem::zeroed::<TomeOwnedStr>() };
				if get_path(&mut owned_str) == TomeStatus::Ok {
					let path_str = tome_owned_to_string(owned_str);
					free_str(owned_str);

					if let Some(path_str) = path_str {
						let path = PathBuf::from(path_str);
						if let Some(parent) = path.parent() {
							cwd = parent.to_path_buf();
						}
					}
				}
			}

			if !cwd.is_absolute()
				&& let Ok(base) = std::env::current_dir()
			{
				cwd = base.join(cwd);
			}
			if let Ok(canon) = cwd.canonicalize() {
				cwd = canon;
			}

			let _ = plugin.cmd_tx.try_send(AgentCommand::Start { cwd });
			TomeStatus::Ok
		} else {
			TomeStatus::Failed
		}
	})
}

pub extern "C" fn command_stop(_ctx: *mut TomeCommandContextV1) -> TomeStatus {
	with_plugin_cmd_tx(|tx| {
		let _ = tx.try_send(AgentCommand::Stop);
	})
}

pub extern "C" fn command_toggle(ctx: *mut TomeCommandContextV1) -> TomeStatus {
	PLUGIN.with(|p_ctx| {
		if let Some(plugin) = p_ctx.borrow().as_ref() {
			let host = unsafe { &*(*ctx).host };
			let mut pid_guard = plugin.state.panel_id.lock();
			let pid = match *pid_guard {
				Some(id) => id,
				None => {
					let id = (host.panel.create)(TomePanelKind::Chat, tome_str("ACP Agent"));
					*pid_guard = Some(id);
					id
				}
			};
			(host.panel.set_open)(pid, TomeBool(1));
			(host.panel.set_focused)(pid, TomeBool(1));
			TomeStatus::Ok
		} else {
			TomeStatus::Failed
		}
	})
}

pub extern "C" fn command_insert_last(ctx: *mut TomeCommandContextV1) -> TomeStatus {
	PLUGIN.with(|p_ctx| {
		if let Some(plugin) = p_ctx.borrow().as_ref() {
			let text = plugin.state.last_assistant_text.lock().clone();
			if !text.is_empty() {
				let host = unsafe { &*(*ctx).host };
				let ts = TomeStr {
					ptr: text.as_ptr(),
					len: text.len(),
				};
				(host.insert_text)(ts);
				TomeStatus::Ok
			} else {
				TomeStatus::Failed
			}
		} else {
			TomeStatus::Failed
		}
	})
}

pub extern "C" fn command_cancel(_ctx: *mut TomeCommandContextV1) -> TomeStatus {
	with_plugin_cmd_tx(|tx| {
		let _ = tx.try_send(AgentCommand::Cancel);
	})
}

fn with_plugin_cmd_tx(f: impl FnOnce(&tokio::sync::mpsc::Sender<AgentCommand>)) -> TomeStatus {
	PLUGIN.with(|ctx| {
		if let Some(plugin) = ctx.borrow().as_ref() {
			f(&plugin.cmd_tx);
			TomeStatus::Ok
		} else {
			TomeStatus::Failed
		}
	})
}
