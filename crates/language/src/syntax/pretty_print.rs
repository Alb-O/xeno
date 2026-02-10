use std::fmt::Write;

use tree_house::tree_sitter::Node;

/// Pretty-prints a syntax tree node for debugging.
pub fn pretty_print_tree<W: Write>(fmt: &mut W, node: Node) -> std::fmt::Result {
	if node.child_count() == 0 {
		if node.is_named() {
			write!(fmt, "({})", node.kind())
		} else {
			write!(
				fmt,
				"\"{}\"",
				node.kind().replace('\\', "\\\\").replace('"', "\\\"")
			)
		}
	} else {
		pretty_print_tree_impl(fmt, &mut node.walk(), 0)
	}
}

/// Recursive implementation of tree pretty-printing.
fn pretty_print_tree_impl<W: Write>(
	fmt: &mut W,
	cursor: &mut tree_house::tree_sitter::TreeCursor,
	depth: usize,
) -> std::fmt::Result {
	let node = cursor.node();
	let visible = node.is_missing()
		|| (node.is_named() && node.grammar().node_kind_is_visible(node.kind_id()));

	if visible {
		let indent = depth * 2;
		write!(fmt, "{:indent$}", "")?;

		if let Some(field) = cursor.field_name() {
			write!(fmt, "{}: ", field)?;
		}

		write!(fmt, "({}", node.kind())?;
	} else {
		write!(
			fmt,
			" \"{}\"",
			node.kind().replace('\\', "\\\\").replace('"', "\\\"")
		)?;
	}

	if cursor.goto_first_child() {
		loop {
			if cursor.node().is_named() || cursor.node().is_missing() {
				fmt.write_char('\n')?;
			}

			pretty_print_tree_impl(fmt, cursor, depth + 1)?;

			if !cursor.goto_next_sibling() {
				break;
			}
		}
		cursor.goto_parent();
	}

	if visible {
		fmt.write_char(')')?;
	}

	Ok(())
}
