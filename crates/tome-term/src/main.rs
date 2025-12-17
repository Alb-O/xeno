mod app;
mod backend;
mod capabilities;
mod cli;
mod editor;
mod render;
mod styles;
mod terminal;
pub mod terminal_panel;
#[cfg(test)]
mod tests;
pub mod theme;
pub mod themes;

use std::io;

use clap::Parser;

use app::run_editor;
use cli::Cli;
use editor::Editor;

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let editor = match cli.file {
        Some(path) => Editor::new(path)?,
        None => Editor::new_scratch(),
    };

    run_editor(editor)
}
