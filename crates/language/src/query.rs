//! Query types for indentation, text objects, and tags.
//!
//! These queries use tree-sitter's S-expression query language to provide
//! structural understanding of code for features beyond highlighting.

use std::iter;

use ropey::RopeSlice;
use tree_house::TREE_SITTER_MATCH_LIMIT;
use tree_house::tree_sitter::query::{InvalidPredicateError, UserPredicate};
use tree_house::tree_sitter::{Grammar, InactiveQueryCursor, Node, Query, RopeInput};

use crate::grammar::query_search_paths;

/// Reads a query file for a language.
///
/// Checks embedded assets first, then filesystem paths for user overrides.
/// Resolves `; inherits` directives via [`tree_house::read_query`].
pub fn read_query(lang: &str, filename: &str) -> String {
	let query_type = filename.strip_suffix(".scm").unwrap_or(filename);

	tree_house::read_query(lang, |query_lang| {
		if let Some(lang_ref) = xeno_registry::LANGUAGES.get(query_lang)
			&& let Some(content) =
				xeno_registry::languages::queries::get_query_text(&lang_ref, query_type)
		{
			return content.to_string();
		}
		for path in query_search_paths() {
			if let Ok(content) = std::fs::read_to_string(path.join(query_lang).join(filename)) {
				return content;
			}
		}
		String::new()
	})
}

/// Query for computing indentation.
///
/// The capture fields are reserved for future indentation computation
/// but are stored during construction for when that feature is implemented.
#[derive(Debug)]
#[allow(dead_code, reason = "captures reserved for future indentation feature")]
pub struct IndentQuery {
	/// The compiled tree-sitter query.
	query: Query,
	/// Capture for nodes that increase indentation.
	indent_capture: Option<tree_house::tree_sitter::Capture>,
	/// Capture for nodes that decrease indentation.
	dedent_capture: Option<tree_house::tree_sitter::Capture>,
	/// Capture for nodes that extend the current indentation scope.
	extend_capture: Option<tree_house::tree_sitter::Capture>,
}

impl IndentQuery {
	/// Compiles an indent query from source.
	pub fn new(
		grammar: Grammar,
		source: &str,
	) -> Result<Self, tree_house::tree_sitter::query::ParseError> {
		let query = Query::new(grammar, source, |_pattern, predicate| {
			// Allow common indent predicates
			match predicate {
				UserPredicate::SetProperty {
					key:
						"indent.begin" | "indent.end" | "indent.dedent" | "indent.branch"
						| "indent.ignore" | "indent.align",
					..
				} => Ok(()),
				_ => Err(InvalidPredicateError::unknown(predicate)),
			}
		})?;

		Ok(Self {
			indent_capture: query.get_capture("indent"),
			dedent_capture: query.get_capture("dedent"),
			extend_capture: query.get_capture("extend"),
			query,
		})
	}

	/// Returns the underlying query.
	pub fn query(&self) -> &Query {
		&self.query
	}
}

/// Query for text object selection.
#[derive(Debug)]
pub struct TextObjectQuery {
	/// The compiled tree-sitter query for text objects.
	query: Query,
}

impl TextObjectQuery {
	/// Compiles a text object query from source.
	pub fn new(
		grammar: Grammar,
		source: &str,
	) -> Result<Self, tree_house::tree_sitter::query::ParseError> {
		let query = Query::new(grammar, source, |_, _| Ok(()))?;
		Ok(Self { query })
	}

	/// Captures nodes matching the given capture name.
	///
	/// Returns an iterator of captured nodes for text object selection.
	pub fn capture_nodes<'a>(
		&'a self,
		capture_name: &str,
		node: &Node<'a>,
		source: RopeSlice<'a>,
	) -> Option<impl Iterator<Item = CapturedNode<'a>>> {
		let capture = self.query.get_capture(capture_name)?;

		let mut cursor = InactiveQueryCursor::new(0..u32::MAX, TREE_SITTER_MATCH_LIMIT)
			.execute_query(&self.query, node, RopeInput::new(source));

		let capture_node = iter::from_fn(move || {
			let mat = cursor.next_match()?;
			Some(mat.nodes_for_capture(capture).cloned().collect())
		})
		.filter_map(|nodes: Vec<_>| {
			if nodes.len() > 1 {
				Some(CapturedNode::Grouped(nodes))
			} else {
				nodes.into_iter().map(CapturedNode::Single).next()
			}
		});

		Some(capture_node)
	}

	/// Captures nodes matching any of the given capture names.
	pub fn capture_nodes_any<'a>(
		&'a self,
		capture_names: &[&str],
		node: &Node<'a>,
		source: RopeSlice<'a>,
	) -> Option<impl Iterator<Item = CapturedNode<'a>>> {
		let capture = capture_names
			.iter()
			.find_map(|name| self.query.get_capture(name))?;

		let mut cursor = InactiveQueryCursor::new(0..u32::MAX, TREE_SITTER_MATCH_LIMIT)
			.execute_query(&self.query, node, RopeInput::new(source));

		let capture_node = iter::from_fn(move || {
			let mat = cursor.next_match()?;
			Some(mat.nodes_for_capture(capture).cloned().collect())
		})
		.filter_map(|nodes: Vec<_>| {
			if nodes.len() > 1 {
				Some(CapturedNode::Grouped(nodes))
			} else {
				nodes.into_iter().map(CapturedNode::Single).next()
			}
		});

		Some(capture_node)
	}
}

/// A captured node or group of nodes from a text object query.
#[derive(Debug)]
pub enum CapturedNode<'a> {
	/// A single captured node.
	Single(Node<'a>),
	/// Multiple captured nodes (from quantified patterns).
	Grouped(Vec<Node<'a>>),
}

impl CapturedNode<'_> {
	/// Returns the start byte position.
	pub fn start_byte(&self) -> usize {
		match self {
			Self::Single(n) => n.start_byte() as usize,
			Self::Grouped(ns) => ns[0].start_byte() as usize,
		}
	}

	/// Returns the end byte position.
	pub fn end_byte(&self) -> usize {
		match self {
			Self::Single(n) => n.end_byte() as usize,
			Self::Grouped(ns) => ns.last().unwrap().end_byte() as usize,
		}
	}

	/// Returns the byte range.
	pub fn byte_range(&self) -> std::ops::Range<usize> {
		self.start_byte()..self.end_byte()
	}
}

/// Query for symbol tags (used for symbol navigation).
#[derive(Debug)]
pub struct TagQuery {
	/// The compiled tree-sitter query for symbol tags.
	pub query: Query,
}

impl TagQuery {
	/// Compiles a tag query from source.
	pub fn new(
		grammar: Grammar,
		source: &str,
	) -> Result<Self, tree_house::tree_sitter::query::ParseError> {
		let query = Query::new(grammar, source, |_pattern, predicate| {
			// Allow tag-specific predicates
			match predicate {
				UserPredicate::IsPropertySet { key: "local", .. } => Ok(()),
				UserPredicate::Other(pred) => match pred.name() {
					"strip!" | "select-adjacent!" => Ok(()),
					_ => Err(InvalidPredicateError::unknown(predicate)),
				},
				_ => Err(InvalidPredicateError::unknown(predicate)),
			}
		})?;

		Ok(Self { query })
	}
}

/// Query for rainbow bracket highlighting.
#[derive(Debug)]
pub struct RainbowQuery {
	/// The compiled tree-sitter query for rainbow brackets.
	pub query: Query,
	/// Capture for nodes that define a nesting scope for bracket coloring.
	pub scope_capture: Option<tree_house::tree_sitter::Capture>,
	/// Capture for bracket characters to be colorized.
	pub bracket_capture: Option<tree_house::tree_sitter::Capture>,
}

impl RainbowQuery {
	/// Compiles a rainbow query from source.
	pub fn new(
		grammar: Grammar,
		source: &str,
	) -> Result<Self, tree_house::tree_sitter::query::ParseError> {
		let query = Query::new(grammar, source, |_pattern, predicate| match predicate {
			UserPredicate::SetProperty {
				key: "rainbow.include-children",
				val,
			} => {
				if val.is_some() {
					return Err(
						"property 'rainbow.include-children' does not take an argument".into(),
					);
				}
				Ok(())
			}
			_ => Err(InvalidPredicateError::unknown(predicate)),
		})?;

		Ok(Self {
			scope_capture: query.get_capture("rainbow.scope"),
			bracket_capture: query.get_capture("rainbow.bracket"),
			query,
		})
	}
}

// Query tests require actual grammars, which are tested in integration tests
