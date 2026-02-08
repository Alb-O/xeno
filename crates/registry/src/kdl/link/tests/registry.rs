use std::collections::HashSet;

use super::super::link_options;
use crate::kdl::loader::{
	load_gutter_metadata, load_hook_metadata, load_option_metadata, load_statusline_metadata,
};

#[test]
fn all_kdl_options_parse_and_validate() {
	let blob = load_option_metadata();
	let validators: Vec<&crate::options::OptionValidatorStatic> =
		inventory::iter::<crate::options::OptionValidatorReg>
			.into_iter()
			.map(|r| r.0)
			.collect();

	// This will panic if any option is invalid (type, default, scope, or unknown validator)
	link_options(&blob, validators.into_iter());
}

#[test]
fn all_kdl_gutters_have_handlers() {
	let blob = load_gutter_metadata();
	let handler_names: HashSet<&str> = inventory::iter::<crate::gutter::GutterHandlerReg>
		.into_iter()
		.map(|r| r.0.name)
		.collect();

	for gutter in &blob.gutters {
		assert!(
			handler_names.contains(gutter.common.name.as_str()),
			"KDL gutter '{}' has no handler",
			gutter.common.name
		);
	}
}

#[test]
fn all_gutter_handlers_have_kdl_entries() {
	let blob = load_gutter_metadata();
	let kdl_names: HashSet<&str> = blob
		.gutters
		.iter()
		.map(|g| g.common.name.as_str())
		.collect();

	for reg in inventory::iter::<crate::gutter::GutterHandlerReg> {
		assert!(
			kdl_names.contains(reg.0.name),
			"gutter_handler!({}) has no KDL entry",
			reg.0.name
		);
	}
}

#[test]
fn all_kdl_segments_have_handlers() {
	let blob = load_statusline_metadata();
	let handler_names: HashSet<&str> =
		inventory::iter::<crate::statusline::handler::StatuslineHandlerReg>
			.into_iter()
			.map(|r| r.0.name)
			.collect();

	for seg in &blob.segments {
		assert!(
			handler_names.contains(seg.common.name.as_str()),
			"KDL segment '{}' has no handler",
			seg.common.name
		);
	}
}

#[test]
fn all_segment_handlers_have_kdl_entries() {
	let blob = load_statusline_metadata();
	let kdl_names: HashSet<&str> = blob
		.segments
		.iter()
		.map(|s| s.common.name.as_str())
		.collect();

	for reg in inventory::iter::<crate::statusline::handler::StatuslineHandlerReg> {
		assert!(
			kdl_names.contains(reg.0.name),
			"segment_handler!({}) has no KDL entry",
			reg.0.name
		);
	}
}

#[test]
fn all_kdl_hooks_have_handlers() {
	let blob = load_hook_metadata();
	let handler_names: HashSet<&str> = inventory::iter::<crate::hooks::HookHandlerReg>
		.into_iter()
		.map(|r| r.0.name)
		.collect();

	for hook in &blob.hooks {
		assert!(
			handler_names.contains(hook.common.name.as_str()),
			"KDL hook '{}' has no handler",
			hook.common.name
		);
	}
}

#[test]
fn all_hook_handlers_have_kdl_entries() {
	let blob = load_hook_metadata();
	let kdl_names: HashSet<&str> = blob.hooks.iter().map(|h| h.common.name.as_str()).collect();

	for reg in inventory::iter::<crate::hooks::HookHandlerReg> {
		assert!(
			kdl_names.contains(reg.0.name),
			"hook_handler!({}) has no KDL entry",
			reg.0.name
		);
	}
}
