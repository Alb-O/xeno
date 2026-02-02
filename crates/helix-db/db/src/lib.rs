pub mod helix_engine;
#[cfg(feature = "server")]
pub mod helix_gateway;
#[cfg(feature = "compiler")]
pub mod helixc;
pub mod protocol;
pub mod utils;

extern crate self as helix_db;

#[cfg(test)]
mod macro_tests;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
