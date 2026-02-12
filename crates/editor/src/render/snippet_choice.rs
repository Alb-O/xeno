use crate::impls::Editor;
use crate::snippet::SnippetChoiceOverlay;

impl Editor {
	/// Returns whether the snippet choice popup should be rendered.
	pub fn snippet_choice_popup_visible(&self) -> bool {
		if self.overlay_interaction().is_open() {
			return false;
		}

		self.overlays()
			.get::<SnippetChoiceOverlay>()
			.is_some_and(|overlay| overlay.active && overlay.buffer_id == self.focused_view() && !overlay.options.is_empty())
	}
}
