#[cfg(test)]
mod suite {
	use std::path::PathBuf;
	use std::sync::Arc;

	use evildoer_api::Editor;
	use evildoer_tui::Terminal;
	use evildoer_tui::backend::TestBackend;
	use insta::assert_snapshot;

	fn test_editor(content: &str) -> Editor {
		let fs = Arc::new(agentfs_sdk::HostFS::new(std::env::current_dir().unwrap()).unwrap());
		Editor::from_content(fs, content.to_string(), Some(PathBuf::from("test.txt")))
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
