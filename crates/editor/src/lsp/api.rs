/// Editor-owned language server configuration.
///
/// This structure allows consumers to configure LSP servers without depending directly
/// on the `xeno-lsp` crate.
#[derive(Debug, Clone)]
pub struct LanguageServerConfig {
	/// Executable path, e.g. "rust-analyzer".
	pub command: String,
	/// Arguments to pass to the executable (excluding the command itself).
	pub args: Vec<String>,
	/// Extra environment variables for the server process.
	pub env: Vec<(String, String)>,
	/// Files/directories that mark the project root.
	/// The registry walks up from the file path to find these markers.
	pub root_markers: Vec<String>,
	/// Request timeout in seconds.
	pub timeout_secs: u64,
	/// Enable snippet support in completions.
	pub enable_snippets: bool,

	/// Optional per-language `initializationOptions` passed during `initialize`.
	pub initialization_options: Option<serde_json::Value>,
	/// Optional per-language settings/config (e.g. for `workspace/didChangeConfiguration`).
	pub settings: Option<serde_json::Value>,
}

impl LanguageServerConfig {
	#[cfg(feature = "lsp")]
	pub(crate) fn into_xeno_lsp(self) -> xeno_lsp::LanguageServerConfig {
		use std::collections::HashMap;
		let env: HashMap<String, String> = self.env.into_iter().collect();

		// Collapse editor API to xeno-lsp's single JSON blob.
		// Prefer `settings` if provided, else `initialization_options`.
		debug_assert!(
			!(self.initialization_options.is_some() && self.settings.is_some()),
			"editor LanguageServerConfig: both initialization_options and settings set; xeno-lsp has only one `config` blob"
		);
		let config = self.settings.or(self.initialization_options);

		xeno_lsp::LanguageServerConfig {
			command: self.command,
			args: self.args,
			env,
			root_markers: self.root_markers,
			timeout_secs: self.timeout_secs,
			config,
			enable_snippets: self.enable_snippets,
		}
	}
}

/// Editor-owned diagnostic information.
#[derive(Debug, Clone)]
pub struct Diagnostic {
	/// Zero-based range in the document (start_line, start_col, end_line, end_col).
	pub range: (usize, usize, usize, usize),
	/// Severity of the diagnostic.
	pub severity: DiagnosticSeverity,
	/// Description message.
	pub message: String,
	/// Optional source of the diagnostic (e.g. "rustc").
	pub source: Option<String>,
	/// Optional error code.
	pub code: Option<String>,
}

/// Diagnostic severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
	Error,
	Warning,
	Info,
	Hint,
}
