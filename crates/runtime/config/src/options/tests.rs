use xeno_registry::options::keys;

use super::*;

fn parse_global(node: &KdlNode) -> Result<ParsedOptions> {
	parse_options_with_context(node, ParseContext::Global)
}

#[test]
fn test_parse_options() {
	let kdl = r##"
options {
    tab-width 4
    theme "gruvbox"
}
"##;
	let doc: kdl::KdlDocument = kdl.parse().unwrap();
	let opts = parse_global(doc.get("options").unwrap()).unwrap().store;
	let options = &options::OPTIONS;

	assert_eq!(
		opts.get(
			options
				.get_key(&keys::TAB_WIDTH.untyped())
				.unwrap()
				.dense_id()
		),
		Some(&OptionValue::Int(4))
	);
	assert_eq!(
		opts.get(options.get_key(&keys::THEME.untyped()).unwrap().dense_id()),
		Some(&OptionValue::String("gruvbox".to_string()))
	);
}

#[test]
fn test_unknown_option_error() {
	let kdl = r##"
options {
    unknown-option 42
}
"##;
	let doc: kdl::KdlDocument = kdl.parse().unwrap();
	let result = parse_global(doc.get("options").unwrap());

	assert!(result.is_err());
	let err = result.unwrap_err();
	assert!(matches!(err, ConfigError::UnknownOption { .. }));
}

#[test]
fn test_unknown_option_with_suggestion() {
	let kdl = r##"
options {
    tab-wdith 4
}
"##;
	let doc: kdl::KdlDocument = kdl.parse().unwrap();
	let result = parse_global(doc.get("options").unwrap());

	assert!(result.is_err());
	if let Err(ConfigError::UnknownOption { key, suggestion }) = result {
		assert_eq!(key, "tab-wdith");
		assert_eq!(suggestion, Some("tab-width".to_string()));
	} else {
		panic!("expected UnknownOption error");
	}
}

#[test]
fn test_type_mismatch_error() {
	let kdl = r##"
options {
    tab-width "four"
}
"##;
	let doc: kdl::KdlDocument = kdl.parse().unwrap();
	let result = parse_global(doc.get("options").unwrap());

	assert!(result.is_err());
	if let Err(ConfigError::OptionTypeMismatch {
		option,
		expected,
		got,
	}) = result
	{
		assert_eq!(option, "tab-width");
		assert_eq!(expected, "int");
		assert_eq!(got, "string");
	} else {
		panic!("expected OptionTypeMismatch error");
	}
}

#[test]
fn test_language_specific_options() {
	use crate::Config;

	let kdl = r##"
options {
    tab-width 4
}

language "rust" {
    tab-width 2
}

language "python" {
    tab-width 8
}
"##;
	let config = Config::parse(kdl).unwrap();
	let options = &options::OPTIONS;

	// Global options
	assert_eq!(
		config.options.get(
			options
				.get_key(&keys::TAB_WIDTH.untyped())
				.unwrap()
				.dense_id()
		),
		Some(&OptionValue::Int(4))
	);

	// Language-specific options
	assert_eq!(config.languages.len(), 2);

	let rust = config.languages.iter().find(|l| l.name == "rust").unwrap();
	assert_eq!(
		rust.options.get(
			options
				.get_key(&keys::TAB_WIDTH.untyped())
				.unwrap()
				.dense_id()
		),
		Some(&OptionValue::Int(2))
	);

	let python = config
		.languages
		.iter()
		.find(|l| l.name == "python")
		.unwrap();
	assert_eq!(
		python.options.get(
			options
				.get_key(&keys::TAB_WIDTH.untyped())
				.unwrap()
				.dense_id()
		),
		Some(&OptionValue::Int(8))
	);
}

#[test]
fn test_global_option_in_language_block_warns() {
	use crate::Config;
	use crate::error::ConfigWarning;

	let kdl = r##"
language "rust" {
    theme "gruvbox"
}
"##;
	let config = Config::parse(kdl).unwrap();

	// Should have a warning about theme in language block
	assert!(!config.warnings.is_empty(), "expected warnings, got none");
	assert!(
		matches!(
			&config.warnings[0],
			ConfigWarning::ScopeMismatch { option, .. } if option == "theme"
		),
		"expected ScopeMismatch warning for 'theme', got: {:?}",
		config.warnings
	);

	// The option should NOT be set in the language store
	let rust = config.languages.iter().find(|l| l.name == "rust").unwrap();
	let options = &options::OPTIONS;
	assert_eq!(
		rust.options
			.get(options.get_key(&keys::THEME.untyped()).unwrap().dense_id()),
		None,
		"global option should not be stored in language scope"
	);
}

#[test]
fn test_buffer_scoped_option_in_language_block_ok() {
	use crate::Config;

	let kdl = r##"
language "rust" {
    tab-width 2
}
"##;
	let config = Config::parse(kdl).unwrap();

	// Should have no warnings - tab-width is buffer-scoped
	assert!(
		config.warnings.is_empty(),
		"unexpected warnings: {:?}",
		config.warnings
	);

	// The option should be set
	let rust = config.languages.iter().find(|l| l.name == "rust").unwrap();
	let options = &options::OPTIONS;
	assert_eq!(
		rust.options.get(
			options
				.get_key(&keys::TAB_WIDTH.untyped())
				.unwrap()
				.dense_id()
		),
		Some(&OptionValue::Int(2))
	);
}
