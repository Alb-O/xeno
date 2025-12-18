#![no_std]
//! Minimal, stable C-ABI surface types for Tome plugins.
//!
//! This crate is the single source of truth for the C ABI:
//! - Used by Tome host to expose the vtables
//! - Used by plugins to consume the ABI without depending on Tome internals
//! - Used by cbindgen to generate `tome_cabi.h`

pub const TOME_C_ABI_VERSION_V2: u32 = 2;

/// Status codes returned across the ABI boundary.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TomeStatus {
	Ok = 0,
	Failed = 1,
	Incompatible = 2,
	/// The operation is not allowed or context is missing.
	///
	/// # Threading & Context
	///
	/// Most host functions require an "active" context (set via `ACTIVE_MANAGER`
	/// and `ACTIVE_EDITOR` TLS). This context is automatically set by the host
	/// during host->guest calls (like `init`, `poll_event`, `on_panel_submit`).
	///
	/// If a plugin calls host functions from a background thread or outside
	/// of a host->guest call, the call may fail with `AccessDenied` or no-op
	/// silently if the function signature does not return `TomeStatus`.
	AccessDenied = 3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TomeStr {
	pub ptr: *const u8,
	pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TomeStrArray {
	pub ptr: *const TomeStr,
	pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TomeBool(pub u8); // 0/1

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A string owned by the creator.
///
/// # Ownership
///
/// The side that allocates the string (host or guest) is responsible for freeing it.
/// If the host passes a `TomeOwnedStr` to the guest (e.g. in `get_current_path`),
/// the guest MUST call `host.free_str` when done.
/// If the guest passes a `TomeOwnedStr` to the host (e.g. in `TomePluginEventV1`),
/// the host MUST call `guest.free_str` when done.
///
/// Never free a string allocated by the other side using your own allocator (e.g. `Box::from_raw`).
/// Always use the provided `free_str` callback.
pub struct TomeOwnedStr {
	pub ptr: *mut u8,
	pub len: usize,
}

pub type TomeFreeStrFn = extern "C" fn(s: TomeOwnedStr);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TomeMessageKind {
	Info = 1,
	Error = 2,
}

#[repr(C)]
pub struct TomeCommandSpecV1 {
	pub name: TomeStr,
	pub aliases: TomeStrArray,
	pub description: TomeStr,
	pub handler: Option<extern "C" fn(ctx: *mut TomeCommandContextV1) -> TomeStatus>,
	pub user_data: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct TomeCommandContextV1 {
	pub argc: usize,
	pub argv: *const TomeStr,
	pub host: *const TomeHostV2,
}

pub type TomePanelId = u64;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TomePanelKind {
	Chat = 1,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TomeChatRole {
	User = 1,
	Assistant = 2,
	System = 3,
	Thought = 4,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TomeHostPanelApiV1 {
	pub create: extern "C" fn(kind: TomePanelKind, title: TomeStr) -> TomePanelId,
	pub set_open: extern "C" fn(id: TomePanelId, open: TomeBool),
	pub set_focused: extern "C" fn(id: TomePanelId, focused: TomeBool),
	pub append_transcript: extern "C" fn(id: TomePanelId, role: TomeChatRole, text: TomeStr),
	pub request_redraw: extern "C" fn(),
}

pub type TomePermissionRequestId = u64;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TomePermissionOptionV1 {
	pub option_id: TomeOwnedStr,
	pub label: TomeOwnedStr,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TomePermissionRequestV1 {
	pub prompt: TomeOwnedStr,
	pub options: *mut TomePermissionOptionV1,
	pub options_len: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TomePluginEventKind {
	PanelAppend = 1,
	PanelSetOpen = 2,
	ShowMessage = 3,
	RequestPermission = 4,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TomePluginEventV1 {
	pub kind: TomePluginEventKind,
	pub panel_id: TomePanelId,
	pub role: TomeChatRole,
	pub text: TomeOwnedStr,
	pub bool_val: TomeBool,
	pub permission_request_id: TomePermissionRequestId,
	pub permission_request: *mut TomePermissionRequestV1,
}

/// Host function table passed to the plugin (V2).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TomeHostV2 {
	pub abi_version: u32,
	pub log: Option<extern "C" fn(msg: TomeStr)>,
	pub panel: TomeHostPanelApiV1,
	pub show_message: extern "C" fn(kind: TomeMessageKind, msg: TomeStr),
	pub insert_text: extern "C" fn(text: TomeStr),
	pub register_command: Option<extern "C" fn(spec: TomeCommandSpecV1)>,
	pub get_current_path: Option<extern "C" fn(out: *mut TomeOwnedStr) -> TomeStatus>,
	pub free_str: Option<extern "C" fn(s: TomeOwnedStr)>,
	pub fs_read_text: Option<extern "C" fn(path: TomeStr, out: *mut TomeOwnedStr) -> TomeStatus>,
	pub fs_write_text: Option<extern "C" fn(path: TomeStr, content: TomeStr) -> TomeStatus>,
}

/// Guest function table returned by the plugin (V2).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TomeGuestV2 {
	pub abi_version: u32,
	pub namespace: TomeStr,
	pub name: TomeStr,
	pub version: TomeStr,
	pub init: Option<extern "C" fn(host: *const TomeHostV2) -> TomeStatus>,
	pub shutdown: Option<extern "C" fn()>,
	pub poll_event: Option<extern "C" fn(out: *mut TomePluginEventV1) -> TomeBool>,
	pub free_str: Option<extern "C" fn(s: TomeOwnedStr)>,
	pub on_panel_submit: Option<extern "C" fn(panel: TomePanelId, text: TomeStr)>,
	pub on_permission_decision:
		Option<extern "C" fn(id: TomePermissionRequestId, option_id: TomeStr)>,
	pub free_permission_request: Option<extern "C" fn(req: *mut TomePermissionRequestV1)>,
}

pub type TomePluginEntryV2 =
	unsafe extern "C" fn(host: *const TomeHostV2, out_guest: *mut TomeGuestV2) -> TomeStatus;
