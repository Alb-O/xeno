//! Build-time infrastructure for compiling NUON assets into binary blobs.
//!
//! Gated behind the `compile` feature. Provides shared utilities used by
//! each domain's `compile` submodule to parse NUON definitions and emit
//! postcard-serialized blob files consumed at runtime.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use walkdir::WalkDir;
use xeno_nu_data::Value;

pub const MAGIC: &[u8; 8] = b"XENOASST";
pub const SCHEMA_VERSION: u32 = 1;

pub struct BuildCtx {
	pub manifest_dir: PathBuf,
	pub out_dir: PathBuf,
}

impl Default for BuildCtx {
	fn default() -> Self {
		Self::new()
	}
}

impl BuildCtx {
	pub fn new() -> Self {
		let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
		let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
		Self { manifest_dir, out_dir }
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
		file.write_all(&SCHEMA_VERSION.to_le_bytes()).expect("failed to write version");
		file.write_all(data).expect("failed to write data");
	}
}

/// Reads a NUON file and parses it into a `xeno_nu_data::Value`.
pub fn read_nuon_value(path: &Path) -> Value {
	let content = fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
	xeno_nu_api::parse_nuon(&content).unwrap_or_else(|e| panic!("failed to parse NUON {}: {e}", path.display()))
}

/// Reads a NUON file and deserializes it into `T` directly from `xeno_nu_data::Value`.
pub fn read_nuon_spec<T: DeserializeOwned>(path: &Path) -> T {
	let value = read_nuon_value(path);
	crate::nu_de::from_nu_value(&value).unwrap_or_else(|e| panic!("failed to deserialize {}: {e}", path.display()))
}

/// Collects all files with the given extension under `root`, sorted by path for determinism.
pub fn collect_files_sorted(root: &Path, ext: &str) -> Vec<PathBuf> {
	let mut paths: Vec<PathBuf> = WalkDir::new(root)
		.into_iter()
		.filter_map(|e| e.ok())
		.filter(|e| e.path().extension().is_some_and(|x| x == ext))
		.map(|e| e.into_path())
		.collect();
	paths.sort();
	paths
}

/// Validates no duplicate names in a list of `(name, _)` pairs.
pub fn validate_unique(items: &[(String, String)], domain: &str) {
	let mut seen = HashSet::new();
	for (name, _) in items {
		if !seen.insert(name.as_str()) {
			panic!("duplicate {domain} name: '{name}'");
		}
	}
}
