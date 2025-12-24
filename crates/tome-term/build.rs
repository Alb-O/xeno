use std::path::Path;
use std::{env, fs};

fn main() {
	let out_dir = env::var("OUT_DIR").unwrap();
	let dest_path = Path::new(&out_dir).join("extensions.rs");

	// Extensions are located in src/extensions/
	let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
	let ext_dir = Path::new(&manifest_dir).join("src").join("extensions");

	let mut extensions = Vec::new();

	if ext_dir.exists() {
		for entry in fs::read_dir(&ext_dir).unwrap() {
			let entry = entry.unwrap();
			let path = entry.path();

			if path.is_dir() {
				if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
					// Only include if it has a mod.rs or if it's a directory we want to treat as a module
					if path.join("mod.rs").exists() || path.join("lib.rs").exists() {
						extensions.push(name.to_string());
					}
				}
			} else if path.is_file() {
				if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
					if path.extension().map(|e| e == "rs").unwrap_or(false) && name != "mod" {
						extensions.push(name.to_string());
					}
				}
			}
		}
	}

	let mut content = String::new();
	for ext in extensions {
		let path = ext_dir.join(&ext);
		if path.is_dir() {
			if path.join("mod.rs").exists() {
				content.push_str(&format!(
					"#[path = \"{}\"]\n",
					path.join("mod.rs").display()
				));
			} else if path.join("lib.rs").exists() {
				content.push_str(&format!(
					"#[path = \"{}\"]\n",
					path.join("lib.rs").display()
				));
			}
		} else {
			content.push_str(&format!("#[path = \"{}\"]\n", path.display()));
		}
		content.push_str(&format!("pub mod {};\n", ext));

		// Emit a cfg flag for this extension
		println!("cargo:rustc-cfg=extension_{}", ext);
	}

	fs::write(&dest_path, content).unwrap();

	// Re-run if the extensions directory changes
	println!("cargo:rerun-if-changed=src/extensions");
}
