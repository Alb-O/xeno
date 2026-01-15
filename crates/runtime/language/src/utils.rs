//! Shared utilities for KDL parsing.

use kdl::KdlDocument;

/// Extracts string arguments from a named child node.
///
/// Given a KDL structure like `args foo bar baz`, returns `["foo", "bar", "baz"]`.
pub fn parse_string_args(children: Option<&KdlDocument>, name: &str) -> Vec<String> {
	children
		.and_then(|c| c.get(name))
		.map(|node| {
			node.entries()
				.iter()
				.filter(|e| e.name().is_none())
				.filter_map(|e| e.value().as_string())
				.map(String::from)
				.collect()
		})
		.unwrap_or_default()
}
