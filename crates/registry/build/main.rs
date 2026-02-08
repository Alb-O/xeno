use std::env;
use std::path::PathBuf;

mod actions;
mod common;
mod registry;
mod themes;
mod types;

use actions::build_actions_blob;
use registry::{
	build_commands_blob, build_gutters_blob, build_hooks_blob, build_motions_blob,
	build_notifications_blob, build_options_blob, build_statusline_blob, build_textobj_blob,
};
use themes::build_themes_blob;

fn main() {
	let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

	let data_dir = PathBuf::from(&manifest_dir)
		.parent()
		.unwrap()
		.join("runtime/data/assets/registry");

	build_actions_blob(&data_dir, &out_dir);
	build_commands_blob(&data_dir, &out_dir);
	build_motions_blob(&data_dir, &out_dir);
	build_textobj_blob(&data_dir, &out_dir);
	build_options_blob(&data_dir, &out_dir);
	build_gutters_blob(&data_dir, &out_dir);
	build_statusline_blob(&data_dir, &out_dir);
	build_hooks_blob(&data_dir, &out_dir);
	build_notifications_blob(&data_dir, &out_dir);

	let themes_dir = PathBuf::from(&manifest_dir)
		.parent()
		.unwrap()
		.join("runtime/data/assets/themes");
	println!("cargo:rerun-if-changed={}", themes_dir.display());
	build_themes_blob(&themes_dir, &out_dir);
}
