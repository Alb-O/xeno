use std::ffi::{CStr, c_char};
use std::path::Path;

use libloading::Library;
use tome_cabi_types::{TOME_C_ABI_VERSION, TomeGuestV1, TomeHostV1, TomePluginEntry, TomeStatus};

/// Errors while loading a C-ABI plugin.
#[derive(Debug)]
pub enum CAbiLoadError {
    Load(libloading::Error),
    MissingEntry,
    Incompatible { host: u32, guest: u32 },
    InitFailed,
}

impl std::fmt::Display for CAbiLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CAbiLoadError::Load(e) => write!(f, "dlopen failed: {e}"),
            CAbiLoadError::MissingEntry => write!(f, "missing entry symbol 'tome_plugin_entry'"),
            CAbiLoadError::Incompatible { host, guest } => {
                write!(f, "incompatible abi version: host={host} guest={guest}")
            }
            CAbiLoadError::InitFailed => write!(f, "plugin init failed"),
        }
    }
}

impl std::error::Error for CAbiLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CAbiLoadError::Load(e) => Some(e),
            _ => None,
        }
    }
}

/// Loaded C-ABI plugin handle.
pub struct CAbiPlugin {
    _lib: Library,
    guest: TomeGuestV1,
}

impl CAbiPlugin {
    /// Call the guest `init` if present.
    pub fn init(&self) -> Result<(), CAbiLoadError> {
        if let Some(init) = self.guest.init {
            let status = init();
            if status != TomeStatus::Ok {
                return Err(CAbiLoadError::InitFailed);
            }
        }
        Ok(())
    }
}

/// Load a C-ABI plugin from a shared library file.
pub fn load_c_abi_plugin(path: &Path) -> Result<CAbiPlugin, CAbiLoadError> {
    let lib = unsafe { Library::new(path) }.map_err(CAbiLoadError::Load)?;

    let entry: libloading::Symbol<TomePluginEntry> = unsafe {
        lib.get(b"tome_plugin_entry\0")
            .map_err(|_| CAbiLoadError::MissingEntry)?
    };

    let host = TomeHostV1 {
        abi_version: TOME_C_ABI_VERSION,
        log: Some(host_log),
    };
    let mut guest = TomeGuestV1::default();

    let status = unsafe { entry(&host, &mut guest) };
    if status != TomeStatus::Ok {
        return Err(CAbiLoadError::InitFailed);
    }

    if guest.abi_version != TOME_C_ABI_VERSION {
        return Err(CAbiLoadError::Incompatible {
            host: TOME_C_ABI_VERSION,
            guest: guest.abi_version,
        });
    }

    let plugin = CAbiPlugin { _lib: lib, guest };
    plugin.init()?;
    Ok(plugin)
}

extern "C" fn host_log(ptr: *const c_char) {
    if ptr.is_null() {
        return;
    }
    if let Ok(msg) = unsafe { CStr::from_ptr(ptr) }.to_str() {
        eprintln!("[tome plugin] {msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::load_c_abi_plugin;
    use std::path::PathBuf;

    #[test]
    fn cabi_loads_demo_plugin() {
        // Expect the demo cabi plugin to be built in release mode before running this test.
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut path = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("tome-core has workspace root")
            .join("target")
            .join("release");

        let filename = if cfg!(target_os = "windows") {
            "demo_cabi_plugin.dll"
        } else if cfg!(target_os = "macos") {
            "libdemo_cabi_plugin.dylib"
        } else {
            "libdemo_cabi_plugin.so"
        };
        path.push(filename);

        assert!(
            path.exists(),
            "demo cabi plugin not found at {:?}; build with `cargo build -p demo-cabi-plugin --release`",
            path
        );

        let plugin = load_c_abi_plugin(&path).expect("should load demo cabi plugin");
        plugin.init().expect("demo cabi plugin init");
    }
}
