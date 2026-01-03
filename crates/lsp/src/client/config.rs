//! Configuration types for language server clients.

use std::collections::HashMap;
use std::path::PathBuf;

use lsp_types::PositionEncodingKind;
use serde_json::Value;

/// Unique identifier for a language server instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LanguageServerId(pub u64);

impl std::fmt::Display for LanguageServerId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "LSP#{}", self.0)
	}
}

/// Offset encoding for LSP positions.
///
/// LSP uses UTF-16 by default, but servers can negotiate different encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OffsetEncoding {
	/// UTF-8 byte offsets.
	Utf8,
	/// UTF-16 code unit offsets (LSP default).
	#[default]
	Utf16,
	/// UTF-32 / Unicode codepoint offsets.
	Utf32,
}

impl OffsetEncoding {
	/// Parse from LSP position encoding kind.
	pub fn from_lsp(kind: &PositionEncodingKind) -> Option<Self> {
		match kind.as_str() {
			"utf-8" => Some(Self::Utf8),
			"utf-16" => Some(Self::Utf16),
			"utf-32" => Some(Self::Utf32),
			_ => None,
		}
	}
}

/// Configuration for starting a language server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
	/// Command to spawn the language server.
	pub command: String,
	/// Arguments to pass to the command.
	pub args: Vec<String>,
	/// Environment variables to set.
	pub env: HashMap<String, String>,
	/// Root path for the workspace.
	pub root_path: PathBuf,
	/// Request timeout in seconds.
	pub timeout_secs: u64,
	/// Optional server-specific configuration.
	pub config: Option<Value>,
}

impl ServerConfig {
	/// Create a new server configuration.
	pub fn new(command: impl Into<String>, root_path: impl Into<PathBuf>) -> Self {
		Self {
			command: command.into(),
			args: Vec::new(),
			env: HashMap::new(),
			root_path: root_path.into(),
			timeout_secs: 30,
			config: None,
		}
	}

	/// Add command line arguments.
	pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
		self.args = args.into_iter().map(Into::into).collect();
		self
	}

	/// Add environment variables.
	pub fn env(
		mut self,
		env: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
	) -> Self {
		self.env = env.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
		self
	}

	/// Set request timeout.
	pub fn timeout(mut self, secs: u64) -> Self {
		self.timeout_secs = secs;
		self
	}

	/// Set server-specific configuration.
	pub fn config(mut self, config: Value) -> Self {
		self.config = Some(config);
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_offset_encoding_from_lsp() {
		assert_eq!(
			OffsetEncoding::from_lsp(&PositionEncodingKind::UTF8),
			Some(OffsetEncoding::Utf8)
		);
		assert_eq!(
			OffsetEncoding::from_lsp(&PositionEncodingKind::UTF16),
			Some(OffsetEncoding::Utf16)
		);
		assert_eq!(
			OffsetEncoding::from_lsp(&PositionEncodingKind::UTF32),
			Some(OffsetEncoding::Utf32)
		);
	}

	#[test]
	fn test_server_config_builder() {
		let config = ServerConfig::new("rust-analyzer", "/home/user/project")
			.args(["--log-file", "/tmp/ra.log"])
			.timeout(60)
			.config(serde_json::json!({"checkOnSave": true}));

		assert_eq!(config.command, "rust-analyzer");
		assert_eq!(config.args, vec!["--log-file", "/tmp/ra.log"]);
		assert_eq!(config.timeout_secs, 60);
		assert!(config.config.is_some());
	}
}
