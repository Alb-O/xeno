use super::*;

fn render_to_lines(tree: KeyTree<'_>, width: u16, height: u16) -> Vec<String> {
	let area = Rect::new(0, 0, width, height);
	let mut buf = Buffer::empty(area);
	tree.render(area, &mut buf);

	(0..height)
		.map(|y| (0..width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
		.collect()
}

#[test]
fn empty_tree_renders_nothing() {
	let tree = KeyTree::new("g", vec![]);
	let lines = render_to_lines(tree, 20, 5);
	assert!(lines.iter().all(|l| l.is_empty()));
}

#[test]
fn root_with_single_child() {
	let children = vec![KeyTreeNode::new("g", "document_start")];
	let tree = KeyTree::new("g", children);
	let lines = render_to_lines(tree, 25, 4);
	assert_eq!(lines[0], "g");
	assert_eq!(lines[1], "│");
	assert!(lines[2].contains("╰─g document_start"));
}

#[test]
fn root_with_multiple_children() {
	let children = vec![KeyTreeNode::new("g", "start"), KeyTreeNode::new("e", "end"), KeyTreeNode::new("h", "home")];
	let tree = KeyTree::new("g", children);
	let lines = render_to_lines(tree, 20, 6);
	assert_eq!(lines[0], "g");
	assert_eq!(lines[1], "│");
	assert!(lines[2].contains("├─g"));
	assert!(lines[3].contains("├─e"));
	assert!(lines[4].contains("╰─h"));
}

#[test]
fn truncates_to_area() {
	use unicode_width::UnicodeWidthStr;
	let children = vec![KeyTreeNode::new("g", "a very long description")];
	let tree = KeyTree::new("g", children);
	let lines = render_to_lines(tree, 12, 4);
	assert!(lines[2].width() <= 12);
}

#[test]
fn renders_with_ancestors() {
	let ancestors = vec![KeyTreeNode::new("b", "Buffer")];
	let children = vec![KeyTreeNode::new("n", "Next"), KeyTreeNode::new("p", "Previous")];
	let tree = KeyTree::new("ctrl-w", children).root_desc("Window").ancestors(ancestors);
	let lines = render_to_lines(tree, 25, 7);
	assert!(lines[0].contains("ctrl-w"));
	assert!(lines[0].contains("Window"));
	assert!(lines[1].contains("╰─b Buffer"));
	assert!(lines[2].contains("│"));
	assert!(lines[3].contains("├─n Next"));
	assert!(lines[4].contains("╰─p Previous"));
}
