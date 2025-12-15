use clap::Parser;
use std::path::PathBuf;

use crate::styles::cli_styles;

#[derive(Parser, Debug)]
#[command(name = "tome")]
#[command(about = "A modal text editor")]
#[command(version)]
#[command(styles = cli_styles())]
pub struct Cli {
    /// File to edit (opens scratch buffer if omitted)
    pub file: Option<PathBuf>,
}
