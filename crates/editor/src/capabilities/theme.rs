use xeno_registry::ThemeAccess;
use xeno_registry::commands::CommandError;

use crate::capabilities::provider::EditorCaps;
use crate::impls::Editor;

impl ThemeAccess for EditorCaps<'_> {
	fn set_theme(&mut self, name: &str) -> Result<(), CommandError> {
		Editor::set_theme(self.ed, name)
	}
}
