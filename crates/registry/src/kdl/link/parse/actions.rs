use crate::actions::BindingMode;
use crate::core::capability::Capability;

pub(crate) fn parse_binding_mode(mode: &str) -> BindingMode {
	match mode {
		"normal" => BindingMode::Normal,
		"insert" => BindingMode::Insert,
		"match" => BindingMode::Match,
		"space" => BindingMode::Space,
		other => super::unknown("binding mode", other),
	}
}

pub(crate) fn parse_capability(name: &str) -> Capability {
	match name {
		"Text" => Capability::Text,
		"Cursor" => Capability::Cursor,
		"Selection" => Capability::Selection,
		"Mode" => Capability::Mode,
		"Messaging" => Capability::Messaging,
		"Edit" => Capability::Edit,
		"Search" => Capability::Search,
		"Undo" => Capability::Undo,
		"FileOps" => Capability::FileOps,
		"Overlay" => Capability::Overlay,
		other => super::unknown("capability", other),
	}
}
