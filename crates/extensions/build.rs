//! Build script for extension discovery.

use std::path::Path;
use std::{env, fs};

/// Discovers and generates module declarations for all extensions in `extensions/`.
///
/// This build script:
/// 1. Scans the `extensions/` directory for Rust modules and files
/// 2. Sanitizes extension names to valid Rust identifiers
/// 3. Validates module names and paths for portability
/// 4. Generates `extensions.rs` with sorted module declarations
fn main() {
	let out_dir = env::var("OUT_DIR").unwrap();
	let dest_path = Path::new(&out_dir).join("extensions.rs");

	// Extensions are located in extensions/ (sibling to src/)
	let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
	let extension_dir = Path::new(&manifest_dir).join("extensions");

	let mut extensions = Vec::new();

	if extension_dir.exists() {
		for entry in fs::read_dir(&extension_dir).unwrap() {
			let entry = entry.unwrap();
			let path = entry.path();

			let name = if path.is_dir() {
				if path.join("mod.rs").exists() || path.join("lib.rs").exists() {
					path.file_name()
						.and_then(|n| n.to_str())
						.map(|s| s.to_string())
				} else {
					None
				}
			} else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
				let stem = path
					.file_stem()
					.and_then(|n| n.to_str())
					.map(|s| s.to_string());
				if stem.as_deref() == Some("mod") {
					None
				} else {
					stem
				}
			} else {
				None
			};

			if let Some(name) = name {
				extensions.push((name, path));
			}
		}
	}

	// 1) Deterministic ordering: sort by name
	extensions.sort_by(|(a, _), (b, _)| a.cmp(b));

	let mut content = String::new();
	let mut extension_names = Vec::new();

	for (raw_name, path) in extensions {
		let mod_name = raw_name.replace(['-', '.'], "_");

		if !mod_name
			.chars()
			.next()
			.map(|c| c.is_alphabetic() || c == '_')
			.unwrap_or(false)
		{
			panic!(
				"Extension directory name '{}' is not a valid Rust identifier (sanitized to '{}')",
				raw_name, mod_name
			);
		}

		let path_str = if path.is_dir() {
			if path.join("mod.rs").exists() {
				format!("{:?}", path.join("mod.rs").display().to_string())
			} else {
				format!("{:?}", path.join("lib.rs").display().to_string())
			}
		} else {
			format!("{:?}", path.display().to_string())
		};

		content.push_str(&format!("#[path = {}]\n", path_str));
		content.push_str(&format!("pub mod {};\n", mod_name));

		println!("cargo:rustc-cfg=extension_{}", mod_name);
		println!("cargo:rerun-if-changed={}", path.display());

		extension_names.push(raw_name);
	}

	content.push_str("\n/// List of all auto-discovered extension names.\n");
	content.push_str(&format!(
		"pub const EXTENSION_NAMES: &[&str] = &{:?};\n",
		extension_names
	));

	fs::write(&dest_path, content).unwrap();
	println!("cargo:rerun-if-changed=extensions");
}
