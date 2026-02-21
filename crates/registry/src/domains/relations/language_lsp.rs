//! Language-to-LSP server reference validation.

/// Validates that every language-referenced LSP server exists in the LSP domain.
pub fn validate_language_lsp_references(
	languages: &crate::languages::LanguagesRegistry,
	lsp_servers: &crate::lsp_servers::LspServersRegistry,
) -> Result<(), Vec<String>> {
	let mut missing = Vec::new();

	for language in languages.snapshot_guard().iter_refs() {
		for &server_sym in language.lsp_servers.iter() {
			let server = language.resolve(server_sym);
			if lsp_servers.get(server).is_none() {
				missing.push(format!("language '{}' references unknown lsp server '{}'", language.id_str(), server));
			}
		}
	}

	if missing.is_empty() {
		return Ok(());
	}

	Err(missing)
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use super::validate_language_lsp_references;
	use crate::core::index::RegistryBuilder;
	use crate::core::{LanguageId, RegistryMetaStatic, RegistrySource};
	use crate::languages::types::LanguageDef;
	use crate::languages::{LanguageEntry, LanguageInput, LanguagesRegistry};
	use crate::lsp_servers::LspServersRegistry;
	use crate::lsp_servers::entry::{LspServerEntry, LspServerInput};
	use crate::symbol::LspServerId;

	static INVALID_LANGUAGE_DEF: LanguageDef = LanguageDef {
		meta: RegistryMetaStatic {
			id: "test::language::invalid_lsp_ref",
			name: "invalid_lsp_ref",
			keys: &[],
			description: "test language with unknown lsp server",
			priority: 0,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
			flags: 0,
		},
		scope: None,
		grammar_name: None,
		injection_regex: None,
		auto_format: false,
		extensions: &[],
		filenames: &[],
		globs: &[],
		shebangs: &[],
		comment_tokens: &[],
		block_comment: None,
		lsp_servers: &["missing-server"],
		roots: &[],
	};

	/// Must reject unresolved language-to-LSP references before catalog publish.
	///
	/// * Enforced in: `validate_language_lsp_references`
	/// * Failure symptom: catalog loads while containing dangling LSP references.
	#[test]
	fn test_unknown_language_lsp_reference_is_reported() {
		let mut language_builder: RegistryBuilder<LanguageInput, LanguageEntry, LanguageId> = RegistryBuilder::new("languages");
		language_builder.push(Arc::new(LanguageInput::Static(INVALID_LANGUAGE_DEF.clone())));
		let languages = LanguagesRegistry::new(language_builder.build());

		let lsp_builder: RegistryBuilder<LspServerInput, LspServerEntry, LspServerId> = RegistryBuilder::new("lsp_servers");
		let lsp_servers = LspServersRegistry::new("lsp_servers", lsp_builder.build());

		let errors = validate_language_lsp_references(&languages, &lsp_servers).expect_err("missing references should fail validation");
		assert_eq!(errors.len(), 1);
		assert!(errors[0].contains("missing-server"));
	}
}
