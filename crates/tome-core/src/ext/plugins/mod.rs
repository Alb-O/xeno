//! C-ABI plugin integration.
//! Runtime plugins are native `cdylib` libraries that implement `tome_plugin_entry`.

#[cfg(feature = "host")]
pub mod cabi;

#[cfg(feature = "host")]
pub use cabi::{CAbiLoadError, CAbiPlugin, load_c_abi_plugin};
#[cfg(feature = "host")]
pub use tome_cabi_types::{
    TOME_C_ABI_VERSION, TomeGuestV1, TomeHostV1, TomePluginEntry, TomeStatus,
};
