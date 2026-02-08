use super::super::*;
use crate::kdl::loader::load_action_metadata;

#[test]
fn all_kdl_actions_have_handlers() {
	use crate::actions::handler::ActionHandlerStatic;
	let blob = load_action_metadata();
	let handlers: Vec<&ActionHandlerStatic> = inventory::iter::<crate::actions::ActionHandlerReg>
		.into_iter()
		.map(|r| r.0)
		.collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();

	for action in &blob.actions {
		assert!(
			handler_names.contains(action.name.as_str()),
			"KDL action '{}' has no handler",
			action.name
		);
	}
}

#[test]
fn all_handlers_have_kdl_entries() {
	let blob = load_action_metadata();
	let kdl_names: HashSet<&str> = blob.actions.iter().map(|a| a.name.as_str()).collect();

	for reg in inventory::iter::<crate::actions::ActionHandlerReg> {
		assert!(
			kdl_names.contains(reg.0.name),
			"handler '{}' has no KDL entry",
			reg.0.name
		);
	}
}

#[test]
fn bindings_parse_correctly() {
	use std::sync::Arc;

	use crate::actions::BindingMode;
	use crate::kdl::types::KeyBindingRaw;

	let raw = vec![
		KeyBindingRaw {
			mode: "normal".into(),
			keys: "g g".into(),
		},
		KeyBindingRaw {
			mode: "insert".into(),
			keys: "esc".into(),
		},
	];
	let bindings = actions::parse_bindings(&raw, Arc::from("test::action"));
	assert_eq!(bindings.len(), 2);
	assert_eq!(bindings[0].mode, BindingMode::Normal);
	assert_eq!(&*bindings[0].keys, "g g");
	assert_eq!(bindings[1].mode, BindingMode::Insert);
	assert_eq!(&*bindings[1].keys, "esc");
}

#[test]
fn capabilities_parse_correctly() {
	use crate::core::capability::Capability;

	assert_eq!(parse::parse_capability("Text"), Capability::Text);
	assert_eq!(parse::parse_capability("Edit"), Capability::Edit);
	assert_eq!(parse::parse_capability("Cursor"), Capability::Cursor);
	assert_eq!(Capability::Selection, Capability::Selection);
	assert_eq!(parse::parse_capability("Mode"), Capability::Mode);
	assert_eq!(parse::parse_capability("Messaging"), Capability::Messaging);
	assert_eq!(parse::parse_capability("Search"), Capability::Search);
	assert_eq!(parse::parse_capability("Undo"), Capability::Undo);
	assert_eq!(parse::parse_capability("FileOps"), Capability::FileOps);
	assert_eq!(parse::parse_capability("Overlay"), Capability::Overlay);
}
