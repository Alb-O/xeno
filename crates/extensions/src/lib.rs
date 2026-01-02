//! Host extensions discovered at build-time from `extensions/`.

include!(concat!(env!("OUT_DIR"), "/extensions.rs"));
