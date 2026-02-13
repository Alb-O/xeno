use xeno_registry::actions::ThemeAccess;
use xeno_registry::commands::CommandError;

use crate::Editor;
use crate::capabilities::provider::EditorCaps;

impl ThemeAccess for EditorCaps<'_> {
	fn set_theme(&mut self, name: &str) -> Result<(), CommandError> {
		Editor::set_theme(self.ed, name)
	}
}
