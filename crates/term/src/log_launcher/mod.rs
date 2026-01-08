//! Log launcher mode - spawn xeno in a new terminal and watch logs in the original.
//!
//! This module implements a "logger launcher" pattern where:
//! 1. The user runs `xeno --log-launch [file]`
//! 2. A new terminal window spawns with xeno running inside
//! 3. The original terminal becomes an interactive log viewer
//!
//! Communication happens via Unix socket - the spawned xeno sends tracing events
//! to the log viewer which formats them in a tracing-tree style with interactive
//! controls for filtering.

mod protocol;
mod sink;
mod terminal;
mod viewer;

pub use sink::SocketLayer;
pub use terminal::spawn_in_terminal;
pub use viewer::run_log_viewer;

/// Environment variable used to pass socket path to child process.
pub const LOG_SINK_ENV: &str = "XENO_LOG_SINK";
