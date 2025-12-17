#![no_std]
//! Minimal, stable C-ABI surface types for Tome plugins.
//!
//! This crate is the single source of truth for the C ABI:
//! - Used by Tome host to expose the vtables
//! - Used by plugins to consume the ABI without depending on Tome internals
//! - Used by cbindgen to generate `tome_cabi.h`

use core::ffi::c_char;

/// ABI version for compatibility checks.
pub const TOME_C_ABI_VERSION: u32 = 1;

/// Status codes returned across the ABI boundary.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TomeStatus {
    Ok = 0,
    Failed = 1,
    Incompatible = 2,
}

/// Host function table passed to the plugin.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TomeHostV1 {
    /// ABI version of the host.
    pub abi_version: u32,
    /// Optional logging hook from guest -> host (UTF-8 string pointer).
    pub log: Option<extern "C" fn(*const c_char)>,
}

/// Guest function table returned by the plugin.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TomeGuestV1 {
    /// ABI version the guest expects.
    pub abi_version: u32,
    /// Optional initialization hook. Called once after load.
    pub init: Option<extern "C" fn() -> TomeStatus>,
}

/// Signature of the plugin entry point.
pub type TomePluginEntry =
    unsafe extern "C" fn(host: *const TomeHostV1, out_guest: *mut TomeGuestV1) -> TomeStatus;
