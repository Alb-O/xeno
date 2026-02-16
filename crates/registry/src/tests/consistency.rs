#[cfg(any(
	feature = "actions",
	feature = "commands",
	feature = "motions",
	feature = "textobj",
	feature = "hooks",
	feature = "statusline",
	feature = "gutter",
))]
use std::collections::HashSet;

#[test]
#[cfg(feature = "actions")]
fn actions_consistency() {
	use crate::actions::handler::ActionHandlerStatic;
	let spec = crate::actions::loader::load_actions_spec();
	let handlers: Vec<&ActionHandlerStatic> = inventory::iter::<crate::actions::ActionHandlerReg>.into_iter().map(|r| r.0).collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();
	let spec_names: HashSet<&str> = spec.actions.iter().map(|a| a.common.name.as_str()).collect();

	for action in &spec.actions {
		assert!(
			handler_names.contains(action.common.name.as_str()),
			"Spec action '{}' has no handler",
			action.common.name
		);
	}

	for handler in &handlers {
		assert!(spec_names.contains(handler.name), "Handler '{}' has no Spec entry", handler.name);
	}
}

#[test]
#[cfg(feature = "commands")]
fn commands_consistency() {
	use crate::commands::handler::CommandHandlerStatic;
	let spec = crate::commands::loader::load_commands_spec();
	let handlers: Vec<&CommandHandlerStatic> = inventory::iter::<crate::commands::CommandHandlerReg>.into_iter().map(|r| r.0).collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();
	let spec_names: HashSet<&str> = spec.commands.iter().map(|c| c.common.name.as_str()).collect();

	for cmd in &spec.commands {
		assert!(
			handler_names.contains(cmd.common.name.as_str()),
			"Spec command '{}' has no handler",
			cmd.common.name
		);
	}

	for handler in &handlers {
		assert!(spec_names.contains(handler.name), "Handler '{}' has no Spec entry", handler.name);
	}
}

// motions
#[test]
#[cfg(feature = "motions")]
fn motions_consistency() {
	use crate::motions::handler::MotionHandlerStatic;
	let spec = crate::motions::loader::load_motions_spec();
	let handlers: Vec<&MotionHandlerStatic> = inventory::iter::<crate::motions::MotionHandlerReg>.into_iter().map(|r| r.0).collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();
	let spec_names: HashSet<&str> = spec.motions.iter().map(|m| m.common.name.as_str()).collect();

	for motion in &spec.motions {
		assert!(
			handler_names.contains(motion.common.name.as_str()),
			"Spec motion '{}' has no handler",
			motion.common.name
		);
	}

	for handler in &handlers {
		assert!(spec_names.contains(handler.name), "Handler '{}' has no Spec entry", handler.name);
	}
}

// textobj
#[test]
#[cfg(feature = "textobj")]
fn textobj_consistency() {
	use crate::textobj::handler::TextObjectHandlerStatic;
	let spec = crate::textobj::loader::load_text_objects_spec();
	let handlers: Vec<&TextObjectHandlerStatic> = inventory::iter::<crate::textobj::TextObjectHandlerReg>.into_iter().map(|r| r.0).collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();
	let spec_names: HashSet<&str> = spec.text_objects.iter().map(|t| t.common.name.as_str()).collect();

	for obj in &spec.text_objects {
		assert!(
			handler_names.contains(obj.common.name.as_str()),
			"Spec textobj '{}' has no handler",
			obj.common.name
		);
	}

	for handler in &handlers {
		assert!(spec_names.contains(handler.name), "Handler '{}' has no Spec entry", handler.name);
	}
}

// hooks
#[test]
#[cfg(feature = "hooks")]
fn hooks_consistency() {
	use crate::hooks::handler::HookHandlerStatic;
	let spec = crate::hooks::loader::load_hooks_spec();
	let handlers: Vec<&HookHandlerStatic> = inventory::iter::<crate::hooks::HookHandlerReg>.into_iter().map(|r| r.0).collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();
	let spec_names: HashSet<&str> = spec.hooks.iter().map(|h| h.common.name.as_str()).collect();

	for hook in &spec.hooks {
		assert!(
			handler_names.contains(hook.common.name.as_str()),
			"Spec hook '{}' has no handler",
			hook.common.name
		);
	}

	for handler in &handlers {
		assert!(spec_names.contains(handler.name), "Handler '{}' has no Spec entry", handler.name);
	}
}

// statusline
#[test]
#[cfg(feature = "statusline")]
fn statusline_consistency() {
	use crate::statusline::handler::StatuslineHandlerStatic;
	let spec = crate::statusline::loader::load_statusline_spec();
	let handlers: Vec<&StatuslineHandlerStatic> = inventory::iter::<crate::statusline::handler::StatuslineHandlerReg>
		.into_iter()
		.map(|r| r.0)
		.collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();
	let spec_names: HashSet<&str> = spec.segments.iter().map(|s| s.common.name.as_str()).collect();

	for seg in &spec.segments {
		assert!(
			handler_names.contains(seg.common.name.as_str()),
			"Spec statusline segment '{}' has no handler",
			seg.common.name
		);
	}

	for handler in &handlers {
		assert!(spec_names.contains(handler.name), "Handler '{}' has no Spec entry", handler.name);
	}
}

// gutter
#[test]
#[cfg(feature = "gutter")]
fn gutter_consistency() {
	use crate::gutter::handler::GutterHandlerStatic;
	let spec = crate::gutter::loader::load_gutters_spec();
	let handlers: Vec<&GutterHandlerStatic> = inventory::iter::<crate::gutter::GutterHandlerReg>.into_iter().map(|r| r.0).collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();
	let spec_names: HashSet<&str> = spec.gutters.iter().map(|g| g.common.name.as_str()).collect();

	for gutter in &spec.gutters {
		assert!(
			handler_names.contains(gutter.common.name.as_str()),
			"Spec gutter '{}' has no handler",
			gutter.common.name
		);
	}

	for handler in &handlers {
		assert!(spec_names.contains(handler.name), "Handler '{}' has no Spec entry", handler.name);
	}
}
