use std::path::PathBuf;
use std::{env, fs};

fn main() {
	let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

	let bindings = cbindgen::generate(&crate_dir).expect("Unable to generate bindings");

	let out_header = out_dir.join("tome_cabi.h");
	bindings.write_to_file(&out_header);

	// Also emit to workspace target/generated for convenient access.
	if let Some(workspace_root) = crate_dir.parent().and_then(|p| p.parent()) {
		let gen_dir = workspace_root.join("target/generated");
		let _ = fs::create_dir_all(&gen_dir);
		let workspace_header = gen_dir.join("tome_cabi.h");
		let _ = fs::copy(&out_header, &workspace_header);
	}

	println!("cargo:rerun-if-changed=src/lib.rs");
}
