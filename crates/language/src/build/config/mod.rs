//! Grammar configuration types loaded from registry specs.

use std::path::PathBuf;

use xeno_registry_spec::grammars::{GrammarSourceSpec, GrammarSpec};

use crate::grammar::{cache_dir, runtime_dir};

/// Grammar configuration from grammars.kdl.
#[derive(Debug, Clone)]
pub struct GrammarConfig {
	/// The grammar name (used for the output library name).
	pub grammar_id: String,
	/// The source location for the grammar.
	pub source: GrammarSource,
}

/// Source location for a grammar.
#[derive(Debug, Clone)]
pub enum GrammarSource {
	/// A local path to the grammar source.
	Local {
		/// Filesystem path to the grammar sources.
		path: String,
	},
	/// A git repository containing the grammar.
	Git {
		/// Git remote URL.
		remote: String,
		/// Git revision (commit hash, tag, or branch).
		revision: String,
		/// Optional subdirectory within the repository.
		subpath: Option<String>,
	},
}

/// Loads grammar configurations from compiled registry specs.
pub fn load_grammar_configs() -> super::Result<Vec<GrammarConfig>> {
	let spec = xeno_registry::domains::grammars::loader::load_grammars_spec();
	Ok(spec.grammars.into_iter().map(GrammarConfig::from).collect())
}

impl From<GrammarSpec> for GrammarConfig {
	fn from(value: GrammarSpec) -> Self {
		let source = match value.source {
			GrammarSourceSpec::Local { path } => GrammarSource::Local { path },
			GrammarSourceSpec::Git { remote, revision, subpath } => GrammarSource::Git { remote, revision, subpath },
		};

		Self { grammar_id: value.id, source }
	}
}

/// Get the directory where grammar sources are stored.
///
/// Grammar sources are stored in the cache directory since they can be
/// re-fetched at any time.
pub fn grammar_sources_dir() -> PathBuf {
	cache_dir().unwrap_or_else(runtime_dir).join("grammars").join("sources")
}

/// Returns the directory where compiled grammars are stored.
pub fn grammar_lib_dir() -> PathBuf {
	if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR")
		&& let Some(workspace) = std::path::Path::new(&manifest).ancestors().nth(2)
	{
		return workspace.join("target").join("grammars");
	}

	cache_dir().map(|c| c.join("grammars")).unwrap_or_else(|| runtime_dir().join("grammars"))
}

/// Get the source directory for a grammar (where parser.c lives).
pub fn get_grammar_src_dir(grammar: &GrammarConfig) -> PathBuf {
	match &grammar.source {
		GrammarSource::Local { path } => PathBuf::from(path).join("src"),
		GrammarSource::Git { subpath, .. } => {
			let base = grammar_sources_dir().join(&grammar.grammar_id);
			match subpath {
				Some(sub) => base.join(sub).join("src"),
				None => base.join("src"),
			}
		}
	}
}

/// Get the library file extension for the current platform.
#[cfg(target_os = "windows")]
pub fn library_extension() -> &'static str {
	"dll"
}

/// Get the library file extension for the current platform.
#[cfg(target_os = "macos")]
pub fn library_extension() -> &'static str {
	"dylib"
}

/// Get the library file extension for the current platform.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn library_extension() -> &'static str {
	"so"
}

#[cfg(test)]
mod tests;
