#![allow(unused_crate_dependencies)]

use std::sync::Arc;

use ropey::Rope;
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::syntax::{SealedSource, Syntax, SyntaxOptions};

#[test]
fn test_highlighter_mapped_offsets() {
	let loader = LanguageLoader::from_embedded();
	let rust_lang = loader.language_for_name("rust").expect("rust has grammar");

	// Create a "document" with some prefix before the window we want to parse
	let prefix = "/* prefix */\n";
	let window_content = "fn main() {\n    let x = 42;\n}";
	let full_content = format!("{}{}", prefix, window_content);
	let source = Rope::from_str(&full_content);

	let base_offset = prefix.len() as u32;
	let real_len = window_content.len() as u32;

	// Create a SealedSource for the window (no suffix for this simple test)
	let sealed = Arc::new(SealedSource::from_window(
		source.byte_slice(base_offset as usize..(base_offset + real_len) as usize),
		"",
	));

	// Create a viewport syntax tree
	let syntax = Syntax::new_viewport(
		sealed,
		rust_lang,
		&loader,
		SyntaxOptions::default(),
		base_offset,
	)
	.expect("Failed to create viewport syntax");

	// Get highlighter for the viewport (doc-global range matching the window)
	let range = base_offset..(base_offset + real_len);
	let highlighter = syntax.highlighter(source.slice(..), &loader, range);

	let spans: Vec<_> = highlighter.collect();

	println!("Viewport spans (base_offset={}):", base_offset);
	for span in &spans {
		let text = source.byte_slice(span.start as usize..span.end as usize);
		println!(
			"  [{:3}-{:3}] highlight={:2} text={:?}",
			span.start,
			span.end,
			span.highlight.idx(),
			text.to_string()
		);
	}

	assert!(!spans.is_empty(), "Should produce highlights for viewport");
	for span in &spans {
		assert!(
			span.start >= base_offset,
			"Span start {} should be >= base_offset {}",
			span.start,
			base_offset
		);
		assert!(
			span.end <= base_offset + real_len,
			"Span end {} should be <= window end {}",
			span.end,
			base_offset + real_len
		);
	}
}

#[test]
fn test_highlighter_full_doc_non_zero_start() {
	let loader = LanguageLoader::from_embedded();
	let rust_lang = loader.language_for_name("rust").expect("rust has grammar");

	let source = Rope::from_str("fn main() {\n    let x = 42;\n}");
	let syntax = Syntax::new(
		source.slice(..),
		rust_lang,
		&loader,
		SyntaxOptions::default(),
	)
	.expect("Failed to create syntax");

	// Highlight starting from a non-zero offset (e.g. after 'fn ')
	let start_offset = 3;
	let highlighter = syntax.highlighter(source.slice(..), &loader, start_offset..);
	let spans: Vec<_> = highlighter.collect();

	println!("Full doc spans (starting at {}):", start_offset);
	for span in &spans {
		let text = source.byte_slice(span.start as usize..span.end as usize);
		println!(
			"  [{:3}-{:3}] highlight={:2} text={:?}",
			span.start,
			span.end,
			span.highlight.idx(),
			text.to_string()
		);
	}

	assert!(
		!spans.is_empty(),
		"Should produce highlights starting from non-zero offset"
	);
	for span in &spans {
		assert!(
			span.start >= start_offset,
			"Span start {} should be >= {}",
			span.start,
			start_offset
		);
	}
}
