use xeno_registry::actions::OptionAccess;
use xeno_registry::options::{OptionKey, OptionValue};

use crate::capabilities::provider::EditorCaps;

impl OptionAccess for EditorCaps<'_> {
	fn option_raw(&self, key: OptionKey) -> OptionValue {
		self.ed.resolve_option(self.ed.focused_view(), key)
	}
}
