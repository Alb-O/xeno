#[cfg(test)]
mod suite {
	use std::path::PathBuf;

	use insta::assert_snapshot;
	use xeno_api::Editor;
	use xeno_tui::Terminal;
	use xeno_tui::backend::TestBackend;

	fn test_editor(content: &str) -> Editor {
		Editor::from_content(content.to_string(), Some(PathBuf::from("test.txt")))
	}

	#[tokio::test]
	async fn test_render_empty() {
		let mut editor = test_editor("");
		let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[tokio::test]
	async fn test_render_with_content() {
		let mut editor = test_editor("Hello, World!\nThis is a test.\nLine 3.");
		let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}
}
