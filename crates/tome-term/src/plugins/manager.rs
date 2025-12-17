use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::panels::{ChatItem, ChatPanelState};
use crate::editor::Editor;
use libloading::{Library, Symbol};
use tome_cabi_types::{
    TOME_C_ABI_VERSION_V2, TomeBool, TomeChatRole, TomeCommandContextV1, TomeCommandSpecV1,
    TomeGuestV2, TomeHostPanelApiV1, TomeHostV2, TomeMessageKind, TomeOwnedStr, TomePanelId,
    TomePanelKind, TomePluginEntryV2, TomeStatus, TomeStr,
};

thread_local! {
    pub(crate) static ACTIVE_MANAGER: RefCell<Option<*mut PluginManager>> = RefCell::new(None);
    pub(crate) static ACTIVE_EDITOR: RefCell<Option<*mut Editor>> = RefCell::new(None);
}

pub struct LoadedPlugin {
    pub lib: Library,
    pub guest: TomeGuestV2,
    pub path: PathBuf,
}

pub struct PluginCommand {
    pub namespace: String,
    pub name: String,
    pub handler: extern "C" fn(ctx: *mut TomeCommandContextV1) -> TomeStatus,
    pub user_data: *mut core::ffi::c_void,
}

pub struct PluginManager {
    pub plugins: Vec<LoadedPlugin>,
    pub commands: HashMap<String, PluginCommand>,
    pub panels: HashMap<u64, ChatPanelState>,
    next_panel_id: u64,
    current_namespace: Option<String>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            commands: HashMap::new(),
            panels: HashMap::new(),
            next_panel_id: 1,
            current_namespace: None,
        }
    }

    pub fn load(&mut self, path: &Path) -> Result<(), String> {
        let lib =
            unsafe { Library::new(path) }.map_err(|e| format!("Failed to load library: {}", e))?;

        let entry: Symbol<TomePluginEntryV2> = unsafe {
            lib.get(b"tome_plugin_entry_v2\0")
                .map_err(|_| "Missing entry symbol 'tome_plugin_entry_v2'")?
        };

        let host = TomeHostV2 {
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
            fs_read_text: None,
            fs_write_text: None,
        };

        let mut guest = unsafe { std::mem::zeroed::<TomeGuestV2>() };

        let status = self.with_active(|_mgr| unsafe { entry(&host, &mut guest) });

        if status != TomeStatus::Ok {
            return Err(format!("Plugin entry failed with status {:?}", status));
        }

        if guest.abi_version != TOME_C_ABI_VERSION_V2 {
            return Err(format!(
                "Incompatible ABI version: host={}, guest={}",
                TOME_C_ABI_VERSION_V2, guest.abi_version
            ));
        }

        let namespace = tome_str_to_str(guest.namespace).to_string();
        self.current_namespace = Some(namespace);

        // Call init
        if let Some(init) = guest.init {
            let status = self.with_active(|_mgr| init(&host));
            if status != TomeStatus::Ok {
                self.current_namespace = None;
                return Err(format!("Plugin init failed with status {:?}", status));
            }
        }

        self.plugins.push(LoadedPlugin {
            lib,
            guest,
            path: path.to_path_buf(),
        });

        self.current_namespace = None;

        Ok(())
    }

    pub fn register_command(&mut self, spec: TomeCommandSpecV1) {
        let namespace = match &self.current_namespace {
            Some(ns) => ns.clone(),
            None => {
                eprintln!("Warning: register_command called outside of plugin init");
                return;
            }
        };

        if let Some(handler) = spec.handler {
            let name = tome_str_to_str(spec.name).to_string();
            let full_name = format!("{}.{}", namespace, name);

            self.commands.insert(
                full_name,
                PluginCommand {
                    namespace: namespace.clone(),
                    name,
                    handler,
                    user_data: spec.user_data,
                },
            );

            // Also handle aliases
            let aliases = unsafe { std::slice::from_raw_parts(spec.aliases.ptr, spec.aliases.len) };
            for alias in aliases {
                let alias_name = tome_str_to_str(*alias).to_string();
                let full_alias = format!("{}.{}", namespace, alias_name);
                self.commands.insert(
                    full_alias,
                    PluginCommand {
                        namespace: namespace.clone(),
                        name: alias_name,
                        handler,
                        user_data: spec.user_data,
                    },
                );
            }
        }
    }

    pub fn with_active<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let old = ACTIVE_MANAGER.with(|ctx| ctx.replace(Some(self as *mut Self)));
        let res = f(self);
        ACTIVE_MANAGER.with(|ctx| ctx.replace(old));
        res
    }

    pub fn autoload(&mut self) {
        let dirs = vec![
            std::env::var("TOME_PLUGIN_DIR").ok().map(PathBuf::from),
            home::home_dir().map(|h| h.join(".config/tome/plugins")),
        ];

        for dir in dirs.into_iter().flatten() {
            if !dir.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if is_dynamic_lib(&path) {
                        if let Err(e) = self.load(&path) {
                            eprintln!("Failed to load plugin {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
    }
}

fn is_dynamic_lib(path: &Path) -> bool {
    let ext = path.extension().and_then(OsStr::to_str);
    match ext {
        Some("so") | Some("dylib") | Some("dll") => true,
        _ => false,
    }
}

pub fn tome_str_to_str<'a>(ts: TomeStr) -> &'a str {
    if ts.ptr.is_null() {
        return "";
    }
    unsafe {
        let slice = std::slice::from_raw_parts(ts.ptr, ts.len);
        std::str::from_utf8_unchecked(slice)
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

// Host callbacks

pub(crate) extern "C" fn host_log(msg: TomeStr) {
    let s = tome_str_to_str(msg);
    eprintln!("[plugin] {}", s);
}

pub(crate) extern "C" fn host_panel_create(kind: TomePanelKind, title: TomeStr) -> TomePanelId {
    ACTIVE_MANAGER.with(|ctx| {
        if let Some(mgr_ptr) = *ctx.borrow() {
            let mgr = unsafe { &mut *mgr_ptr };
            let id = mgr.next_panel_id;
            mgr.next_panel_id += 1;

            let title_str = tome_str_to_str(title).to_string();
            match kind {
                TomePanelKind::Chat => {
                    mgr.panels.insert(id, ChatPanelState::new(id, title_str));
                }
            }
            id
        } else {
            0
        }
    })
}

pub(crate) extern "C" fn host_panel_set_open(id: TomePanelId, open: TomeBool) {
    ACTIVE_MANAGER.with(|ctx| {
        if let Some(mgr_ptr) = *ctx.borrow() {
            let mgr = unsafe { &mut *mgr_ptr };
            if let Some(panel) = mgr.panels.get_mut(&id) {
                panel.open = open.0 != 0;
            }
        }
    })
}

pub(crate) extern "C" fn host_panel_set_focused(id: TomePanelId, focused: TomeBool) {
    ACTIVE_MANAGER.with(|ctx| {
        if let Some(mgr_ptr) = *ctx.borrow() {
            let mgr = unsafe { &mut *mgr_ptr };
            if let Some(panel) = mgr.panels.get_mut(&id) {
                panel.focused = focused.0 != 0;
                if panel.focused {
                    for (pid, p) in &mut mgr.panels {
                        if *pid != id {
                            p.focused = false;
                        }
                    }
                }
            }
        }
    })
}

pub(crate) extern "C" fn host_panel_append_transcript(
    id: TomePanelId,
    role: TomeChatRole,
    text: TomeStr,
) {
    ACTIVE_MANAGER.with(|ctx| {
        if let Some(mgr_ptr) = *ctx.borrow() {
            let mgr = unsafe { &mut *mgr_ptr };
            if let Some(panel) = mgr.panels.get_mut(&id) {
                panel.transcript.push(ChatItem {
                    role,
                    text: tome_str_to_str(text).to_string(),
                });
            }
        }
    })
}

pub(crate) extern "C" fn host_request_redraw() {}

pub(crate) extern "C" fn host_show_message(kind: TomeMessageKind, msg: TomeStr) {
    let s = tome_str_to_str(msg);
    ACTIVE_EDITOR.with(|ctx| {
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
    let s = tome_str_to_str(text);
    ACTIVE_EDITOR.with(|ctx| {
        if let Some(ed_ptr) = *ctx.borrow() {
            let ed = unsafe { &mut *ed_ptr };
            ed.insert_text(s);
        }
    })
}

pub(crate) extern "C" fn host_register_command(spec: TomeCommandSpecV1) {
    ACTIVE_MANAGER.with(|ctx| {
        if let Some(mgr_ptr) = *ctx.borrow() {
            let mgr = unsafe { &mut *mgr_ptr };
            mgr.register_command(spec);
        }
    })
}
