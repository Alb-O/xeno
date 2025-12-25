#[cfg(test)]
mod suite {
	use std::path::PathBuf;
	use std::sync::Arc;

	use insta::assert_snapshot;
	use ratatui::Terminal;
	use ratatui::backend::TestBackend;
	use tome_api::Editor;
	use tome_manifest::{CommandContext, CommandOutcome};
	use tome_theme::{CMD_THEME, DEFAULT_THEME_ID, THEMES, get_theme};

	fn test_editor(content: &str) -> Editor {
		let fs = Arc::new(agentfs_sdk::HostFS::new(std::env::current_dir().unwrap()).unwrap());
		Editor::from_content(fs, content.to_string(), Some(PathBuf::from("test.txt")))
	}

	#[test]
	fn test_themes_registry() {
		assert!(THEMES.len() >= 5);

		let default = get_theme("default");
		assert!(default.is_some());

		let solarized = get_theme("solarized_dark");
		assert!(solarized.is_some());
		assert_eq!(get_theme("solarized").unwrap().name, "solarized_dark");
		assert_eq!(get_theme("solarized-dark").unwrap().name, "solarized_dark");

		let monokai = get_theme("monokai");
		assert!(monokai.is_some());
		assert_eq!(get_theme("monokai-extended").unwrap().name, "monokai");

		let one_dark = get_theme("one_dark");
		assert!(one_dark.is_some());
		assert_eq!(get_theme("onedark").unwrap().name, "one_dark");
		assert_eq!(get_theme("one").unwrap().name, "one_dark");

		let gruvbox = get_theme("gruvbox");
		assert!(gruvbox.is_some());
		assert_eq!(get_theme("gruvbox-dark").unwrap().name, "gruvbox");
	}

	#[tokio::test]
	async fn test_theme_command() {
		let mut editor = Editor::new_scratch();

		assert_eq!(editor.theme.id, DEFAULT_THEME_ID);

		let args = ["default"];
		let mut ctx = CommandContext {
			editor: &mut editor,
			args: &args,
			count: 1,
			register: None,
			user_data: CMD_THEME.user_data,
		};

		let result = (CMD_THEME.handler)(&mut ctx).await;
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), CommandOutcome::Ok);

		assert_eq!(editor.theme.name, "default");

		let args_typo = ["gruv-box"];
		let mut ctx_typo = CommandContext {
			editor: &mut editor,
			args: &args_typo,
			count: 1,
			register: None,
			user_data: CMD_THEME.user_data,
		};

		let result_typo = (CMD_THEME.handler)(&mut ctx_typo).await;
		assert!(result_typo.is_err());
		if let Err(tome_manifest::CommandError::Failed(msg)) = result_typo {
			assert!(msg.contains(&format!("Did you mean '{}'?", DEFAULT_THEME_ID)));
		} else {
			panic!("Expected Failed error with suggestion");
		}
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
