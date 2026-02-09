use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

pub const MAGIC: &[u8; 8] = b"XENOASST";
pub const SCHEMA_VERSION: u32 = 1;

pub struct BuildCtx {
	pub manifest_dir: PathBuf,
	pub out_dir: PathBuf,
}

impl BuildCtx {
	pub fn new() -> Self {
		let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
		let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
		Self {
			manifest_dir,
			out_dir,
		}
	}

	pub fn asset(&self, rel: &str) -> PathBuf {
		self.manifest_dir.join(rel)
	}

	pub fn rerun_if_changed(&self, path: &Path) {
		println!("cargo:rerun-if-changed={}", path.display());
	}

	pub fn rerun_tree(&self, root: &Path) {
		println!("cargo:rerun-if-changed={}", root.display());
		for entry in WalkDir::new(root) {
			let entry = entry.unwrap();
			if entry.path().is_file() {
				self.rerun_if_changed(entry.path());
			}
		}
	}

	pub fn write_blob(&self, filename: &str, data: &[u8]) {
		let path = self.out_dir.join(filename);
		let mut file = fs::File::create(&path).expect("failed to create blob");
		file.write_all(MAGIC).expect("failed to write magic");
		file.write_all(&SCHEMA_VERSION.to_le_bytes())
			.expect("failed to write version");
		file.write_all(data).expect("failed to write data");
	}
}

/// Extracts the first positional string argument from a KDL node.
pub fn node_name_arg(node: &kdl::KdlNode, domain: &str) -> String {
	node.entry(0)
		.and_then(|e| {
			if e.name().is_none() {
				e.value().as_string().map(String::from)
			} else {
				None
			}
		})
		.unwrap_or_else(|| panic!("{domain} node missing name argument"))
}

/// Extracts a required string attribute.
pub fn require_str(node: &kdl::KdlNode, attr: &str, context: &str) -> String {
	node.get(attr)
		.and_then(|v| v.as_string())
		.unwrap_or_else(|| panic!("{context} missing '{attr}' attribute"))
		.to_string()
}

/// Extracts positional string arguments from a child node.
pub fn collect_keys(node: &kdl::KdlNode) -> Vec<String> {
	let Some(children) = node.children() else {
		return Vec::new();
	};
	let Some(keys_node) = children.get("keys") else {
		return Vec::new();
	};
	keys_node
		.entries()
		.iter()
		.filter(|e| e.name().is_none())
		.filter_map(|e| e.value().as_string().map(String::from))
		.collect()
}

/// Validates no duplicate names in a list.
pub fn validate_unique(items: &[(String, String)], domain: &str) {
	let mut seen = HashSet::new();
	for (name, _) in items {
		if !seen.insert(name.as_str()) {
			panic!("duplicate {domain} name: '{name}'");
		}
	}
}
