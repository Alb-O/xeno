use lsp_types::PositionEncodingKind;

use super::*;

#[test]
fn test_offset_encoding_from_lsp() {
	assert_eq!(OffsetEncoding::from_lsp(&PositionEncodingKind::UTF8), Some(OffsetEncoding::Utf8));
	assert_eq!(OffsetEncoding::from_lsp(&PositionEncodingKind::UTF16), Some(OffsetEncoding::Utf16));
	assert_eq!(OffsetEncoding::from_lsp(&PositionEncodingKind::UTF32), Some(OffsetEncoding::Utf32));
}

#[test]
fn test_server_config_builder() {
	let id = LanguageServerId::new(0, 1);
	let config = ServerConfig::new(id, "rust-analyzer", "/home/user/project")
		.args(["--log-file", "/tmp/ra.log"])
		.timeout(60)
		.config(serde_json::json!({"checkOnSave": true}));

	assert_eq!(config.id, id);
	assert_eq!(config.command, "rust-analyzer");
	assert_eq!(config.args, vec!["--log-file", "/tmp/ra.log"]);
	assert_eq!(config.timeout_secs, 60);
	assert!(config.config.is_some());
}
