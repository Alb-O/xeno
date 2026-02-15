#![allow(unused_crate_dependencies)]
//! Binary entrypoint for the Iced frontend.

use xeno_editor_iced::{StartupOptions, run};

fn main() -> iced::Result {
	run(StartupOptions::from_env())
}
