//! Host extensions discovered at build-time from `builtins/`.

include!(concat!(env!("OUT_DIR"), "/extensions.rs"));
