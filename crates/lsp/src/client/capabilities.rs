//! Client capabilities for LSP initialization.

use lsp_types::{
	ClientCapabilities, CompletionClientCapabilities, CompletionItemCapability, CompletionItemCapabilityResolveSupport, DiagnosticClientCapabilities,
	GeneralClientCapabilities, HoverClientCapabilities, MarkupKind, PositionEncodingKind, PublishDiagnosticsClientCapabilities, RenameClientCapabilities,
	SignatureHelpClientCapabilities, SignatureInformationSettings, TagSupport, TextDocumentClientCapabilities, WindowClientCapabilities,
	WorkspaceClientCapabilities,
};

/// Build client capabilities for initialization.
pub fn client_capabilities(enable_snippets: bool) -> ClientCapabilities {
	ClientCapabilities {
		workspace: Some(WorkspaceClientCapabilities {
			configuration: Some(true),
			did_change_configuration: Some(lsp_types::DynamicRegistrationClientCapabilities {
				dynamic_registration: Some(false),
			}),
			workspace_folders: Some(true),
			apply_edit: Some(true),
			symbol: Some(lsp_types::WorkspaceSymbolClientCapabilities {
				dynamic_registration: Some(false),
				..Default::default()
			}),
			execute_command: Some(lsp_types::DynamicRegistrationClientCapabilities {
				dynamic_registration: Some(false),
			}),
			inlay_hint: Some(lsp_types::InlayHintWorkspaceClientCapabilities { refresh_support: Some(true) }),
			workspace_edit: Some(lsp_types::WorkspaceEditClientCapabilities {
				document_changes: Some(true),
				resource_operations: Some(vec![
					lsp_types::ResourceOperationKind::Create,
					lsp_types::ResourceOperationKind::Rename,
					lsp_types::ResourceOperationKind::Delete,
				]),
				failure_handling: Some(lsp_types::FailureHandlingKind::Abort),
				normalizes_line_endings: Some(false),
				change_annotation_support: None,
			}),
			did_change_watched_files: Some(lsp_types::DidChangeWatchedFilesClientCapabilities {
				dynamic_registration: Some(false),
				relative_pattern_support: Some(false),
			}),
			file_operations: Some(lsp_types::WorkspaceFileOperationsClientCapabilities {
				will_create: Some(true),
				did_create: Some(true),
				will_rename: Some(true),
				did_rename: Some(true),
				will_delete: Some(true),
				did_delete: Some(true),
				..Default::default()
			}),
			semantic_tokens: Some(lsp_types::SemanticTokensWorkspaceClientCapabilities { refresh_support: Some(true) }),
			diagnostic: Some(lsp_types::DiagnosticWorkspaceClientCapabilities { refresh_support: Some(true) }),
			..Default::default()
		}),
		text_document: Some(TextDocumentClientCapabilities {
			completion: Some(CompletionClientCapabilities {
				completion_item: Some(CompletionItemCapability {
					snippet_support: Some(enable_snippets),
					resolve_support: Some(CompletionItemCapabilityResolveSupport {
						properties: vec![String::from("documentation"), String::from("detail"), String::from("additionalTextEdits")],
					}),
					insert_replace_support: Some(true),
					deprecated_support: Some(true),
					tag_support: Some(TagSupport {
						value_set: vec![lsp_types::CompletionItemTag::DEPRECATED],
					}),
					..Default::default()
				}),
				completion_item_kind: Some(lsp_types::CompletionItemKindCapability { ..Default::default() }),
				context_support: None,
				..Default::default()
			}),
			hover: Some(HoverClientCapabilities {
				content_format: Some(vec![MarkupKind::Markdown]),
				..Default::default()
			}),
			signature_help: Some(SignatureHelpClientCapabilities {
				signature_information: Some(SignatureInformationSettings {
					documentation_format: Some(vec![MarkupKind::Markdown]),
					parameter_information: Some(lsp_types::ParameterInformationSettings {
						label_offset_support: Some(true),
					}),
					active_parameter_support: Some(true),
				}),
				..Default::default()
			}),
			rename: Some(RenameClientCapabilities {
				dynamic_registration: Some(false),
				prepare_support: Some(true),
				prepare_support_default_behavior: None,
				honors_change_annotations: Some(false),
			}),
			formatting: Some(lsp_types::DocumentFormattingClientCapabilities {
				dynamic_registration: Some(false),
			}),
			code_action: Some(lsp_types::CodeActionClientCapabilities {
				code_action_literal_support: Some(lsp_types::CodeActionLiteralSupport {
					code_action_kind: lsp_types::CodeActionKindLiteralSupport {
						value_set: [
							lsp_types::CodeActionKind::EMPTY,
							lsp_types::CodeActionKind::QUICKFIX,
							lsp_types::CodeActionKind::REFACTOR,
							lsp_types::CodeActionKind::REFACTOR_EXTRACT,
							lsp_types::CodeActionKind::REFACTOR_INLINE,
							lsp_types::CodeActionKind::REFACTOR_REWRITE,
							lsp_types::CodeActionKind::SOURCE,
							lsp_types::CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
							lsp_types::CodeActionKind::SOURCE_FIX_ALL,
						]
						.iter()
						.map(|kind| kind.as_str().to_string())
						.collect(),
					},
				}),
				is_preferred_support: Some(true),
				disabled_support: Some(true),
				data_support: Some(true),
				resolve_support: Some(lsp_types::CodeActionCapabilityResolveSupport {
					properties: vec!["edit".to_owned(), "command".to_owned()],
				}),
				..Default::default()
			}),
			diagnostic: Some(DiagnosticClientCapabilities {
				dynamic_registration: Some(false),
				related_document_support: Some(true),
			}),
			publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
				version_support: Some(true),
				tag_support: Some(TagSupport {
					value_set: vec![lsp_types::DiagnosticTag::UNNECESSARY, lsp_types::DiagnosticTag::DEPRECATED],
				}),
				..Default::default()
			}),
			semantic_tokens: Some(lsp_types::SemanticTokensClientCapabilities {
				dynamic_registration: Some(false),
				requests: lsp_types::SemanticTokensClientCapabilitiesRequests {
					full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
					range: Some(true),
					..Default::default()
				},
				token_types: vec![
					lsp_types::SemanticTokenType::NAMESPACE,
					lsp_types::SemanticTokenType::TYPE,
					lsp_types::SemanticTokenType::CLASS,
					lsp_types::SemanticTokenType::ENUM,
					lsp_types::SemanticTokenType::INTERFACE,
					lsp_types::SemanticTokenType::STRUCT,
					lsp_types::SemanticTokenType::TYPE_PARAMETER,
					lsp_types::SemanticTokenType::PARAMETER,
					lsp_types::SemanticTokenType::VARIABLE,
					lsp_types::SemanticTokenType::PROPERTY,
					lsp_types::SemanticTokenType::ENUM_MEMBER,
					lsp_types::SemanticTokenType::EVENT,
					lsp_types::SemanticTokenType::FUNCTION,
					lsp_types::SemanticTokenType::METHOD,
					lsp_types::SemanticTokenType::MACRO,
					lsp_types::SemanticTokenType::KEYWORD,
					lsp_types::SemanticTokenType::MODIFIER,
					lsp_types::SemanticTokenType::COMMENT,
					lsp_types::SemanticTokenType::STRING,
					lsp_types::SemanticTokenType::NUMBER,
					lsp_types::SemanticTokenType::REGEXP,
					lsp_types::SemanticTokenType::OPERATOR,
					lsp_types::SemanticTokenType::DECORATOR,
				],
				token_modifiers: vec![
					lsp_types::SemanticTokenModifier::DECLARATION,
					lsp_types::SemanticTokenModifier::DEFINITION,
					lsp_types::SemanticTokenModifier::READONLY,
					lsp_types::SemanticTokenModifier::STATIC,
					lsp_types::SemanticTokenModifier::DEPRECATED,
					lsp_types::SemanticTokenModifier::ABSTRACT,
					lsp_types::SemanticTokenModifier::ASYNC,
					lsp_types::SemanticTokenModifier::MODIFICATION,
					lsp_types::SemanticTokenModifier::DOCUMENTATION,
					lsp_types::SemanticTokenModifier::DEFAULT_LIBRARY,
				],
				formats: vec![lsp_types::TokenFormat::RELATIVE],
				overlapping_token_support: Some(false),
				multiline_token_support: Some(false),
				server_cancel_support: Some(false),
				augments_syntax_tokens: Some(true),
			}),
			inlay_hint: Some(lsp_types::InlayHintClientCapabilities {
				dynamic_registration: Some(false),
				resolve_support: Some(lsp_types::InlayHintResolveClientCapabilities {
					properties: vec![
						"label.location".to_owned(),
						"label.command".to_owned(),
						"label.tooltip".to_owned(),
						"tooltip".to_owned(),
						"textEdits".to_owned(),
					],
				}),
			}),
			..Default::default()
		}),
		window: Some(WindowClientCapabilities {
			work_done_progress: Some(true),
			show_document: Some(lsp_types::ShowDocumentClientCapabilities { support: false }),
			..Default::default()
		}),
		general: Some(GeneralClientCapabilities {
			position_encodings: Some(vec![PositionEncodingKind::UTF8, PositionEncodingKind::UTF32, PositionEncodingKind::UTF16]),
			..Default::default()
		}),
		..Default::default()
	}
}
