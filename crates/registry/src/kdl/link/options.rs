use super::*;
use crate::kdl::types::OptionsBlob;
use crate::options::def::{LinkedOptionDef, OptionScope};
use crate::options::{OptionDefault, OptionType, OptionValidatorStatic, OptionValue};

/// Links KDL option metadata with validator statics, producing `LinkedOptionDef`s.
pub fn link_options(
	metadata: &OptionsBlob,
	validators: impl Iterator<Item = &'static OptionValidatorStatic>,
) -> Vec<LinkedOptionDef> {
	let validator_map: HashMap<&str, &OptionValidatorStatic> =
		validators.map(|v| (v.name, v)).collect();

	let mut defs = Vec::new();

	for meta in &metadata.options {
		let id = format!("xeno-registry::{}", meta.name);

		let value_type = match meta.value_type.as_str() {
			"bool" => OptionType::Bool,
			"int" => OptionType::Int,
			"string" => OptionType::String,
			other => panic!("unknown option value-type: '{}'", other),
		};

		let scope = match meta.scope.as_str() {
			"global" => OptionScope::Global,
			"buffer" => OptionScope::Buffer,
			other => panic!("unknown option scope: '{}'", other),
		};

		let default = match value_type {
			OptionType::Bool => {
				let val = match meta.default.as_str() {
					"#true" | "true" => true,
					"#false" | "false" => false,
					other => panic!("invalid bool default: '{}'", other),
				};
				OptionDefault::Value(OptionValue::Bool(val))
			}
			OptionType::Int => {
				let val = meta
					.default
					.parse::<i64>()
					.unwrap_or_else(|_| panic!("invalid int default: '{}'", meta.default));
				OptionDefault::Value(OptionValue::Int(val))
			}
			OptionType::String => OptionDefault::Value(OptionValue::String(meta.default.clone())),
		};

		let validator = meta.validator.as_ref().map(|name| {
			validator_map
				.get(name.as_str())
				.map(|v| v.validator)
				.unwrap_or_else(|| {
					panic!(
						"KDL option '{}' references unknown validator '{}'",
						meta.name, name
					)
				})
		});

		defs.push(LinkedOptionDef {
			id,
			name: meta.name.clone(),
			description: meta.description.clone(),
			keys: meta.keys.clone(),
			priority: meta.priority,
			flags: meta.flags,
			kdl_key: meta.kdl_key.clone(),
			value_type,
			default,
			scope,
			validator,
			source: RegistrySource::Builtin,
		});
	}

	defs
}
