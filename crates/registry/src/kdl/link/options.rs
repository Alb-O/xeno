use crate::core::LinkedDef;
use crate::kdl::types::OptionsBlob;
use crate::options::def::{LinkedOptionDef, OptionPayload};
use crate::options::{OptionDefault, OptionType, OptionValidatorStatic, OptionValue};

/// Links KDL option metadata with validator statics, producing `LinkedOptionDef`s.
pub fn link_options(
	metadata: &OptionsBlob,
	validators: impl Iterator<Item = &'static OptionValidatorStatic>,
) -> Vec<LinkedOptionDef> {
	let validator_map = super::common::build_name_map(validators, |v| v.name);

	let mut defs = Vec::new();

	for meta in &metadata.options {
		let value_type = super::parse::parse_option_type(&meta.value_type);
		let scope = super::parse::parse_option_scope(&meta.scope);
		let default = match value_type {
			OptionType::Bool => OptionDefault::Value(OptionValue::Bool(
				super::parse::parse_boolish(&meta.default),
			)),
			OptionType::Int => OptionDefault::Value(OptionValue::Int(super::parse::parse_i64(
				&meta.default,
				"int default",
			))),
			OptionType::String => OptionDefault::Value(OptionValue::String(meta.default.clone())),
		};

		let validator = meta.validator.as_deref().map(|name| {
			validator_map
				.get(name)
				.map(|v| v.validator)
				.unwrap_or_else(|| {
					panic!(
						"KDL option '{}' references unknown validator '{}'",
						meta.common.name, name
					)
				})
		});

		defs.push(LinkedDef {
			meta: super::common::linked_meta_from_common(&meta.common),
			payload: OptionPayload {
				kdl_key: meta.kdl_key.clone(),
				value_type,
				default,
				scope,
				validator,
			},
		});
	}

	defs
}
