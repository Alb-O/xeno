//! Integration tests for xeno-language syntax highlighting.
//!
//! These tests verify the complete pipeline from language registration
//! through syntax parsing to highlight span generation.
//!
//! NOTE: Full syntax highlighting requires compiled tree-sitter grammars.
//! Without grammars, tests verify the API works but can't produce highlights.
//! To get grammars, run: `XENO_RUNTIME=runtime xeno grammar fetch && xeno grammar build`

use ropey::Rope;
use xeno_runtime_language::grammar::{grammar_search_paths, load_grammar};
use xeno_runtime_language::highlight::{Highlight, HighlightStyles};
use xeno_runtime_language::syntax::Syntax;
use xeno_runtime_language::{LanguageData, LanguageLoader};

fn create_test_loader() -> LanguageLoader {
	let mut loader = LanguageLoader::new();
	let rust = LanguageData::new(
		"rust".to_string(),
		None,
		vec!["rs".to_string()],
		vec![],
		vec![],
		vec![],
		vec!["//".to_string()],
		Some(("/*".to_string(), "*/".to_string())),
		Some("rust"),
	);
	loader.register(rust);
	loader
}

#[test]
fn test_language_registration() {
	let loader = create_test_loader();

	assert!(
		loader.language_for_name("rust").is_some(),
		"Should find rust language by name"
	);

	assert!(
		loader
			.language_for_path(std::path::Path::new("test.rs"))
			.is_some(),
		"Should find rust language by .rs extension"
	);

	assert!(
		loader.language_for_name("unknown").is_none(),
		"Should not find unknown language"
	);
}

#[test]
fn test_language_data_fields() {
	let loader = create_test_loader();

	let lang = loader.language_for_name("rust").unwrap();
	let data = loader.get(lang).unwrap();

	assert_eq!(data.name, "rust");
	assert_eq!(data.grammar_name, "rust");
	assert_eq!(data.extensions, vec!["rs"]);
	assert_eq!(data.comment_tokens, vec!["//"]);
	assert_eq!(
		data.block_comment,
		Some(("/*".to_string(), "*/".to_string()))
	);
}

#[test]
fn test_syntax_config_loading() {
	let loader = create_test_loader();

	let lang = loader.language_for_name("rust").unwrap();
	let data = loader.get(lang).unwrap();

	// Try to load syntax config - this will fail if grammar isn't installed
	// but we can at least verify the method exists and doesn't panic
	let config = data.syntax_config();

	// Log whether we have a grammar available
	if config.is_some() {
		println!("Rust grammar loaded successfully!");
	} else {
		println!("Rust grammar not available (expected in CI without grammars)");
	}
}

#[test]
fn test_highlight_styles_creation() {
	use xeno_base::{Color, Style};

	let scopes = ["keyword", "function", "string", "comment"];

	let styles = HighlightStyles::new(&scopes, |scope| match scope {
		"keyword" => Style::new().fg(Color::Red),
		"function" => Style::new().fg(Color::Blue),
		"string" => Style::new().fg(Color::Green),
		"comment" => Style::new().fg(Color::Gray),
		_ => Style::new(),
	});

	assert_eq!(styles.len(), 4);
	assert!(!styles.is_empty());
}

#[test]
fn test_highlight_styles_resolution() {
	use xeno_base::{Color, Style};

	let scopes = ["keyword", "function"];

	let styles = HighlightStyles::new(&scopes, |scope| match scope {
		"keyword" => Style::new().fg(Color::Red),
		"function" => Style::new().fg(Color::Blue),
		_ => Style::new(),
	});

	let keyword_style = styles.style_for_highlight(Highlight::new(0));
	let function_style = styles.style_for_highlight(Highlight::new(1));
	let unknown_style = styles.style_for_highlight(Highlight::new(99));

	assert_eq!(keyword_style.fg, Some(Color::Red));
	assert_eq!(function_style.fg, Some(Color::Blue));
	assert_eq!(unknown_style.fg, None); // Out of bounds returns default
}

#[test]
fn test_syntax_creation_without_grammar() {
	let loader = create_test_loader();
	let source = Rope::from_str("fn main() { println!(\"Hello\"); }");

	let lang = loader.language_for_name("rust").unwrap();

	// Try to create syntax - may fail without grammar
	let syntax = Syntax::new(source.slice(..), lang, &loader);

	if let Ok(syntax) = syntax {
		println!("Syntax created successfully!");

		// Verify we can access the tree
		let tree = syntax.tree();
		println!("Parse tree root: {:?}", tree.root_node().kind());
	} else {
		println!(
			"Syntax creation failed (expected without grammar): {:?}",
			syntax.err()
		);
	}
}

#[test]
fn test_grammar_loading_debug() {
	// Debug test to understand grammar loading
	println!("Grammar search paths:");
	for path in grammar_search_paths() {
		println!("  {:?} (exists: {})", path, path.exists());
		let grammar_path = path.join("rust.so");
		println!(
			"    rust.so: {:?} (exists: {})",
			grammar_path,
			grammar_path.exists()
		);
	}

	// Try to load the grammar directly
	println!("\nAttempting to load rust grammar...");
	match load_grammar("rust") {
		Ok(grammar) => println!("Grammar loaded successfully! {:?}", grammar),
		Err(e) => println!("Grammar loading failed: {:?}", e),
	}
}

#[test]
fn test_full_highlighting_pipeline() {
	use xeno_base::{Color, Style};

	let mut loader = LanguageLoader::new();

	let rust = LanguageData::new(
		"rust".to_string(),
		None,
		vec!["rs".to_string()],
		vec![],
		vec![],
		vec![],
		vec!["//".to_string()],
		Some(("/*".to_string(), "*/".to_string())),
		Some("rust"),
	);
	let rust_lang = loader.register(rust);

	let source = Rope::from_str("fn main() {\n    let x = 42;\n}");

	let syntax = match Syntax::new(source.slice(..), rust_lang, &loader) {
		Ok(s) => s,
		Err(e) => {
			println!("Skipping highlight test - no grammar available: {:?}", e);
			return;
		}
	};

	// Create highlight styles
	// Use actual Helix-style scope names from highlights.scm
	let styles = HighlightStyles::new(
		&[
			"keyword",
			"keyword.control",
			"keyword.function",
			"function",
			"function.method",
			"variable",
			"variable.other.member",
			"type",
			"string",
			"number",
			"operator",
		],
		|scope| match scope {
			s if s.starts_with("keyword") => Style::new().fg(Color::Red),
			s if s.starts_with("function") => Style::new().fg(Color::Blue),
			s if s.starts_with("variable") => Style::new().fg(Color::Yellow),
			s if s.starts_with("type") => Style::new().fg(Color::Green),
			s if s.starts_with("string") => Style::new().fg(Color::Magenta),
			"number" => Style::new().fg(Color::Cyan),
			"operator" => Style::new().fg(Color::White),
			_ => Style::new(),
		},
	);

	// Get highlighter for full document
	let highlighter = syntax.highlighter(source.slice(..), &loader, ..);

	// Collect all highlight spans
	let spans: Vec<_> = highlighter.collect();

	println!("Found {} highlight spans", spans.len());
	for span in &spans {
		let text = source.slice(span.start as usize..span.end as usize);
		let style = styles.style_for_highlight(span.highlight);
		println!(
			"  [{}-{}] {:?} -> {:?}",
			span.start,
			span.end,
			text.to_string(),
			style.fg
		);
	}

	// We should have at least some highlights if grammar loaded
	assert!(!spans.is_empty(), "Should produce highlight spans");
}

#[test]
fn test_language_loader_tree_house_trait() {
	// Verify LanguageLoader implements tree_house::LanguageLoader
	fn assert_language_loader<T: tree_house::LanguageLoader>() {}
	assert_language_loader::<LanguageLoader>();
}

/// Tests that tree-sitter syntax trees are correctly updated incrementally.
///
/// This verifies the core incremental parsing flow:
/// 1. Parse initial document to create syntax tree
/// 2. Apply an insertion via Transaction and update tree incrementally
/// 3. Apply a deletion via Transaction and update tree incrementally
/// 4. Verify tree structure remains valid after each edit
///
/// The test uses a minimal Rust source (`fn main() {}`) and inserts/deletes
/// a let statement to exercise the ChangeSet-to-InputEdit conversion.
#[test]
fn test_incremental_syntax_update() {
	use xeno_base::{Selection, Transaction};

	let mut loader = LanguageLoader::new();

	let rust = LanguageData::new(
		"rust".to_string(),
		None,
		vec!["rs".to_string()],
		vec![],
		vec![],
		vec![],
		vec!["//".to_string()],
		Some(("/*".to_string(), "*/".to_string())),
		Some("rust"),
	);
	let rust_lang = loader.register(rust);

	let mut source = Rope::from_str("fn main() {}");

	let mut syntax = match Syntax::new(source.slice(..), rust_lang, &loader) {
		Ok(s) => s,
		Err(e) => {
			println!(
				"Skipping incremental update test - no grammar available: {:?}",
				e
			);
			return;
		}
	};

	let root = syntax.tree().root_node();
	assert_eq!(root.kind(), "source_file");
	let initial_child_count = root.child_count();

	let old_source = source.clone();
	let insert_pos = 11;
	let selection = Selection::point(insert_pos);
	let tx = Transaction::insert(source.slice(..), &selection, " let x = 42;".to_string());
	tx.apply(&mut source);

	syntax
		.update_from_changeset(
			old_source.slice(..),
			source.slice(..),
			tx.changes(),
			&loader,
		)
		.expect("Incremental update should succeed");

	let root = syntax.tree().root_node();
	assert_eq!(root.kind(), "source_file");
	assert!(
		root.child_count() >= initial_child_count,
		"Tree should reflect the insertion"
	);

	let after_insert = source.to_string();
	assert_eq!(after_insert, "fn main() { let x = 42;}");

	let old_source = source.clone();
	let delete_selection = Selection::single(11, 23);
	let tx = Transaction::delete(source.slice(..), &delete_selection);
	tx.apply(&mut source);

	syntax
		.update_from_changeset(
			old_source.slice(..),
			source.slice(..),
			tx.changes(),
			&loader,
		)
		.expect("Delete update should succeed");

	let after_delete = source.to_string();
	assert_eq!(after_delete, "fn main() {}");

	println!("Incremental syntax updates work correctly!");
}

/// Tests that rustdoc injection highlighting works correctly.
///
/// This tests the injection chain:
/// 1. Rust parses doc comments (`///`)
/// 2. rust/injections.scm injects `markdown-rustdoc` for doc comment content
/// 3. markdown-rustdoc inherits from markdown, injects `rust` for code fences
/// 4. markdown/injections.scm injects `markdown.inline` for inline content
/// 5. markdown.inline highlights links with `@markup.link.label`
#[test]
fn test_rustdoc_injection_chain() {
	// Use the embedded loader which has all language configurations
	let loader = LanguageLoader::from_embedded();

	let rust_lang = loader.language_for_name("rust").unwrap();

	// Test source with doc comments containing links and code
	let source = Rope::from_str(
		r#"/// A simple function.
///
/// # Examples
///
/// ```
/// // Create a layout with fill proportional sizes for each element
/// let x = 42;
/// ```
///
/// See [`main`] for more info.
fn main() {}
"#,
	);

	let syntax = match Syntax::new(source.slice(..), rust_lang, &loader) {
		Ok(s) => s,
		Err(e) => {
			println!(
				"Skipping rustdoc injection test - no grammar available: {:?}",
				e
			);
			return;
		}
	};

	println!("=== Rustdoc Injection Chain Test ===");
	println!("Source:\n{}", source);

	// Check what layers exist
	println!("\n--- Layers ---");
	let mut layer_count = 0;
	for layer in syntax.layers_for_byte_range(0, source.len_bytes() as u32) {
		let layer_data = syntax.layer(layer);
		let lang = loader.get(layer_data.language);
		println!(
			"Layer {}: language={:?} (id={})",
			layer_count,
			lang.map(|l| &l.name),
			layer_data.language.idx()
		);
		layer_count += 1;
	}

	// Get highlights
	let highlighter = syntax.highlighter(source.slice(..), &loader, ..);
	let spans: Vec<_> = highlighter.collect();

	println!("\n--- Highlight Spans ---");
	for span in &spans {
		let text = source.byte_slice(span.start as usize..span.end as usize);
		println!(
			"  bytes [{:3}-{:3}] highlight={:2} text={:?}",
			span.start,
			span.end,
			span.highlight.idx(),
			text.to_string().chars().take(40).collect::<String>()
		);
	}

	// Check if we have markdown.inline language loaded
	println!("\n--- Language Checks ---");
	println!(
		"markdown-rustdoc: {:?}",
		loader.language_for_name("markdown-rustdoc").is_some()
	);
	println!(
		"markdown.inline: {:?}",
		loader.language_for_name("markdown.inline").is_some()
	);
	println!(
		"markdown: {:?}",
		loader.language_for_name("markdown").is_some()
	);

	// Look for the link text [`main`]
	// It should be somewhere around byte position 100-110
	let link_start = source.to_string().find("[`main`]").unwrap();
	let link_end = link_start + "[`main`]".len();
	println!(
		"\nLink [`main`] is at bytes {}-{}: {:?}",
		link_start,
		link_end,
		source.byte_slice(link_start..link_end).to_string()
	);

	// Check what layers cover the link
	println!("\n--- Layers covering the link ---");
	for layer in syntax.layers_for_byte_range(link_start as u32, link_end as u32) {
		let layer_data = syntax.layer(layer);
		let lang = loader.get(layer_data.language);
		println!(
			"  Layer: language={:?} (id={})",
			lang.map(|l| &l.name),
			layer_data.language.idx()
		);
	}

	// Check highlights at the link position
	let link_spans: Vec<_> = spans
		.iter()
		.filter(|s| s.start <= link_start as u32 && s.end > link_start as u32)
		.collect();
	println!("\n--- Spans covering link start ---");
	for span in link_spans {
		let text = source.byte_slice(span.start as usize..span.end as usize);
		println!(
			"  bytes [{:3}-{:3}] highlight={:2} text={:?}",
			span.start,
			span.end,
			span.highlight.idx(),
			text.to_string()
		);
	}

	// Check if markdown.inline has syntax_config
	println!("\n--- Config Checks ---");
	if let Some(md_inline_lang) = loader.language_for_name("markdown.inline") {
		let md_inline_data = loader.get(md_inline_lang).unwrap();
		let has_config = md_inline_data.syntax_config().is_some();
		println!(
			"markdown.inline (lang_id={}) has syntax_config: {}",
			md_inline_lang.idx(),
			has_config
		);
		if has_config {
			println!("  grammar_name: {}", md_inline_data.grammar_name);
		}
	}

	if let Some(md_rustdoc_lang) = loader.language_for_name("markdown-rustdoc") {
		let md_rustdoc_data = loader.get(md_rustdoc_lang).unwrap();
		let has_config = md_rustdoc_data.syntax_config().is_some();
		println!(
			"markdown-rustdoc (lang_id={}) has syntax_config: {}",
			md_rustdoc_lang.idx(),
			has_config
		);
		if has_config {
			println!("  grammar_name: {}", md_rustdoc_data.grammar_name);
		}
	}

	// Print the rust tree
	println!("\n--- Rust tree around doc comments ---");
	let rust_tree = syntax.tree();
	fn print_rust_tree(node: tree_house::tree_sitter::Node, depth: usize, max_depth: usize) {
		if depth > max_depth {
			return;
		}
		let indent = "  ".repeat(depth);
		if node.kind().contains("comment")
			|| node.kind() == "source_file"
			|| node.kind() == "function_item"
		{
			println!(
				"{}{} [{}-{}]",
				indent,
				node.kind(),
				node.start_byte(),
				node.end_byte()
			);
			for i in 0..node.child_count() {
				if let Some(child) = node.child(i) {
					print_rust_tree(child, depth + 1, max_depth);
				}
			}
		} else {
			// Just recurse without printing
			for i in 0..node.child_count() {
				if let Some(child) = node.child(i) {
					print_rust_tree(child, depth, max_depth);
				}
			}
		}
	}
	print_rust_tree(rust_tree.root_node(), 0, 5);

	// Check ALL layers in the document
	println!("\n--- All layers in document ---");
	for layer in syntax.layers_for_byte_range(0, source.len_bytes() as u32) {
		let layer_data = syntax.layer(layer);
		let lang_id = layer_data.language;
		let lang_name = loader
			.get(lang_id)
			.map(|d| d.name.as_str())
			.unwrap_or("unknown");
		if let Some(tree) = layer_data.tree() {
			println!(
				"  Layer {}: {} [{}-{}]",
				lang_id.idx(),
				lang_name,
				tree.root_node().start_byte(),
				tree.root_node().end_byte()
			);
		}
	}

	// Check via tree_house::LanguageLoader trait
	use tree_house::LanguageLoader as TreeHouseLoader;
	println!("\n--- tree_house::LanguageLoader.get_config checks ---");
	for layer in syntax.layers_for_byte_range(link_start as u32, link_end as u32) {
		let layer_data = syntax.layer(layer);
		let lang_id = layer_data.language;
		let has_config = loader.get_config(lang_id).is_some();
		let lang_name = loader
			.get(lang_id)
			.map(|d| d.name.as_str())
			.unwrap_or("unknown");
		let has_tree = layer_data.tree().is_some();
		println!(
			"  Layer lang_id={}: {} -> get_config: {}, has_tree: {}",
			lang_id.idx(),
			lang_name,
			has_config,
			has_tree
		);
		if has_tree {
			let tree = layer_data.tree().unwrap();
			let root = tree.root_node();
			println!(
				"    Tree root: {} [{}-{}]",
				root.kind(),
				root.start_byte(),
				root.end_byte()
			);

			// Print the tree structure for markdown.inline
			if lang_name == "markdown.inline" {
				fn print_tree(node: tree_house::tree_sitter::Node, depth: usize) {
					let indent = "  ".repeat(depth);
					println!(
						"{}    {} {} [{}-{}]",
						indent,
						node.kind(),
						if node.is_named() { "(named)" } else { "" },
						node.start_byte(),
						node.end_byte()
					);
					for i in 0..node.child_count() {
						if let Some(child) = node.child(i) {
							print_tree(child, depth + 1);
						}
					}
				}
				print_tree(root, 0);
			}
		}
	}
}

/// Tests that highlight spans have correct byte positions for doc comments.
///
/// This specifically tests the case where `//!` doc comments should have
/// the entire comment (including the `//!` prefix) highlighted as a comment,
/// not just the text after the prefix.
#[test]
fn test_highlight_span_positions_doc_comment() {
	let mut loader = LanguageLoader::new();

	let rust = LanguageData::new(
		"rust".to_string(),
		None,
		vec!["rs".to_string()],
		vec![],
		vec![],
		vec![],
		vec!["//".to_string()],
		Some(("/*".to_string(), "*/".to_string())),
		Some("rust"),
	);
	let rust_lang = loader.register(rust);

	let source = Rope::from_str("//! Hello world\nfn main() {}");

	let syntax = match Syntax::new(source.slice(..), rust_lang, &loader) {
		Ok(s) => s,
		Err(e) => {
			println!(
				"Skipping highlight span test - no grammar available: {:?}",
				e
			);
			return;
		}
	};

	let highlighter = syntax.highlighter(source.slice(..), &loader, ..);
	let spans: Vec<_> = highlighter.collect();

	println!("Source: {:?}", source.to_string());
	println!("Highlight spans:");
	for span in &spans {
		let text = source.byte_slice(span.start as usize..span.end as usize);
		println!(
			"  bytes [{:2}-{:2}] highlight={:2} text={:?}",
			span.start,
			span.end,
			span.highlight.idx(),
			text.to_string()
		);
	}

	// Find the span that covers the doc comment
	// The `//!` should be at byte 0, and the comment should start there
	let comment_spans: Vec<_> = spans
		.iter()
		.filter(|s| {
			s.start == 0
				|| source
					.byte_slice(s.start as usize..s.end as usize)
					.to_string()
					.starts_with("//")
		})
		.collect();

	println!("\nComment-related spans:");
	for span in &comment_spans {
		let text = source.byte_slice(span.start as usize..span.end as usize);
		println!(
			"  bytes [{:2}-{:2}] text={:?}",
			span.start,
			span.end,
			text.to_string()
		);
	}

	// The first span should start at byte 0 and include "//!"
	// This is the key assertion - if highlights are offset, this will fail
	let first_span = spans.first().expect("Should have at least one span");
	assert_eq!(
		first_span.start, 0,
		"First highlight span should start at byte 0, not {}",
		first_span.start
	);

	let first_text = source
		.byte_slice(first_span.start as usize..first_span.end as usize)
		.to_string();
	assert!(
		first_text.starts_with("//"),
		"First span should contain the comment prefix '//', got: {:?}",
		first_text
	);
}
