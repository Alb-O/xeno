#[cfg(test)]
mod suite {
	use std::path::PathBuf;
	use std::sync::Arc;

	use insta::assert_snapshot;
	use evildoer_api::Editor;
	use evildoer_manifest::{CommandContext, CommandOutcome};
	use evildoer_theme::{CMD_THEME, DEFAULT_THEME_ID, THEMES, get_theme};
	use evildoer_tui::Terminal;
	use evildoer_tui::backend::TestBackend;

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

		// Test switching to "default" theme
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

		// Test that "gruv-box" normalizes to "gruvbox" (dashes are ignored)
		let args_normalized = ["gruv-box"];
		let mut ctx_normalized = CommandContext {
			editor: &mut editor,
			args: &args_normalized,
			count: 1,
			register: None,
			user_data: CMD_THEME.user_data,
		};

		let result_normalized = (CMD_THEME.handler)(&mut ctx_normalized).await;
		assert!(result_normalized.is_ok());
		assert_eq!(editor.theme.name, "gruvbox");

		// Test that a completely unknown theme fails with suggestion
		let args_unknown = ["nonexistent_theme"];
		let mut ctx_unknown = CommandContext {
			editor: &mut editor,
			args: &args_unknown,
			count: 1,
			register: None,
			user_data: CMD_THEME.user_data,
		};

		let result_unknown = (CMD_THEME.handler)(&mut ctx_unknown).await;
		assert!(result_unknown.is_err());
		if let Err(evildoer_manifest::CommandError::Failed(msg)) = result_unknown {
			assert!(msg.contains("Theme not found"));
		} else {
			panic!("Expected Failed error");
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
