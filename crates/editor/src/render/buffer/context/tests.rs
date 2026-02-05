#[cfg(test)]
mod tests {
	#![allow(clippy::module_inception)]
	use xeno_primitives::ViewId;
	use xeno_tui::Terminal;
	use xeno_tui::backend::TestBackend;
	use xeno_tui::layout::Rect;
	use xeno_tui::widgets::Paragraph;

	use crate::buffer::Buffer;
	use crate::render::BufferRenderContext;
	use crate::window::GutterSelector;

	#[test]
	fn test_render_baseline() {
		let buffer = Buffer::new(ViewId::text(1), "Hello world".to_string(), None);
		let theme = xeno_registry::themes::get_theme("default").unwrap();
		let loader = xeno_runtime_language::LanguageLoader::from_embedded();
		let syntax_manager = crate::syntax_manager::SyntaxManager::default();

		let ctx = BufferRenderContext {
			theme: &*theme,
			language_loader: &loader,
			syntax_manager: &syntax_manager,
			diagnostics: None,
			diagnostic_ranges: None,
		};

		let area = Rect::new(0, 0, 20, 3);
		let mut cache = crate::render::cache::RenderCache::new();
		let result = ctx.render_buffer_with_gutter(
			crate::render::buffer::context::types::RenderBufferParams {
				buffer: &buffer,
				area,
				use_block_cursor: true,
				is_focused: true,
				gutter: GutterSelector::Registry,
				tab_width: 4,
				cursorline: false,
				cache: &mut cache,
			},
		);

		let backend = TestBackend::new(20, 3);
		let mut terminal = Terminal::new(backend).unwrap();

		terminal
			.draw(|f| {
				let gutter_area = Rect {
					width: result.gutter_width,
					height: area.height,
					x: area.x,
					y: area.y,
				};
				let text_area = Rect {
					x: area.x + result.gutter_width,
					width: area.width.saturating_sub(result.gutter_width),
					height: area.height,
					y: area.y,
				};

				f.render_widget(Paragraph::new(result.gutter), gutter_area);
				f.render_widget(Paragraph::new(result.text), text_area);
			})
			.unwrap();

		let tui_buffer = terminal.backend().buffer();

		assert!(result.gutter_width > 0);

		let line0: String = (0..20)
			.map(|x| tui_buffer[(x, 0)].symbol().to_string())
			.collect();
		assert!(line0.contains("Hello world"));
		assert!(line0.contains("1"));

		let line1: String = (0..20)
			.map(|x| tui_buffer[(x, 1)].symbol().to_string())
			.collect();
		assert!(line1.contains("~"));
	}

	#[test]
	fn test_render_wrapping() {
		// Use a very narrow text width to force wrapping
		let buffer = Buffer::new(ViewId::text(1), "One two three four five".to_string(), None);
		let theme = xeno_registry::themes::get_theme("default").unwrap();
		let loader = xeno_runtime_language::LanguageLoader::from_embedded();
		let syntax_manager = crate::syntax_manager::SyntaxManager::default();

		let ctx = BufferRenderContext {
			theme: &*theme,
			language_loader: &loader,
			syntax_manager: &syntax_manager,
			diagnostics: None,
			diagnostic_ranges: None,
		};

		// 30 width, gutter will take ~6, leaving ~24 for text
		let area = Rect::new(0, 0, 30, 5);
		let mut cache = crate::render::cache::RenderCache::new();
		let result = ctx.render_buffer_with_gutter(
			crate::render::buffer::context::types::RenderBufferParams {
				buffer: &buffer,
				area,
				use_block_cursor: true,
				is_focused: true,
				gutter: GutterSelector::Registry,
				tab_width: 4,
				cursorline: false,
				cache: &mut cache,
			},
		);

		let backend = TestBackend::new(30, 5);
		let mut terminal = Terminal::new(backend).unwrap();

		terminal
			.draw(|f| {
				let gutter_area = Rect {
					width: result.gutter_width,
					height: area.height,
					x: area.x,
					y: area.y,
				};
				let text_area = Rect {
					x: area.x + result.gutter_width,
					width: area.width.saturating_sub(result.gutter_width),
					height: area.height,
					y: area.y,
				};

				f.render_widget(Paragraph::new(result.gutter), gutter_area);
				f.render_widget(Paragraph::new(result.text), text_area);
			})
			.unwrap();

		let tui_buffer = terminal.backend().buffer();

		// Row 0: Gutter "1 ", text "One two three four five"
		let row0: String = (0..30)
			.map(|x| tui_buffer[(x, 0)].symbol().to_string())
			.collect();
		assert!(row0.contains("1"));
		assert!(row0.contains("One two three four five"));
	}
}
