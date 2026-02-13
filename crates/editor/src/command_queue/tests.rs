use super::*;

#[test]
fn queue_push_and_drain() {
	let mut queue = CommandQueue::new();

	queue.push("lsp-hover", vec![]);
	queue.push("lsp-goto-definition", vec!["--include-declaration".into()]);

	let commands: Vec<_> = queue.drain().collect();
	assert_eq!(commands.len(), 2);
	assert_eq!(commands[0].name, "lsp-hover");
	assert_eq!(commands[1].name, "lsp-goto-definition");
}
