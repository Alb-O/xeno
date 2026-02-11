use super::spec::OptionsSpec;
use crate::core::{LinkedDef, LinkedPayload, OptionType, OptionValue, RegistryMeta, Symbol};
use crate::options::entry::OptionEntry;
use crate::options::{OptionDefault, OptionScope, OptionValidator, OptionValidatorStatic};

/// An option definition assembled from spec + Rust validator.
pub type LinkedOptionDef = LinkedDef<OptionPayload>;

#[derive(Clone)]
pub struct OptionPayload {
	pub kdl_key: String,
	pub value_type: OptionType,
	pub default: OptionDefault,
	pub scope: OptionScope,
	pub validator: Option<OptionValidator>,
}

impl LinkedPayload<OptionEntry> for OptionPayload {
	fn collect_extra_keys<'b>(&'b self, collector: &mut crate::core::index::StringCollector<'_, 'b>) {
		collector.push(self.kdl_key.as_str());
	}

	fn build_entry(&self, ctx: &mut dyn crate::core::index::BuildCtx, meta: RegistryMeta, _short_desc: Symbol) -> OptionEntry {
		OptionEntry {
			meta,
			kdl_key: ctx.intern(&self.kdl_key),
			value_type: self.value_type,
			default: self.default.clone(),
			scope: self.scope,
			validator: self.validator,
		}
	}
}

/// Links option specs with validator statics, producing `LinkedOptionDef`s.
pub fn link_options(spec: &OptionsSpec, validators: impl Iterator<Item = &'static OptionValidatorStatic>) -> Vec<LinkedOptionDef> {
	let validator_map = crate::defs::link::build_name_map(validators, |v| v.name);

	let mut defs = Vec::new();

	for meta in &spec.options {
		let value_type = parse_option_type(&meta.value_type);
		let scope = parse_option_scope(&meta.scope);
		let default = match value_type {
			OptionType::Bool => OptionDefault::Value(OptionValue::Bool(parse_boolish(&meta.default))),
			OptionType::Int => OptionDefault::Value(OptionValue::Int(parse_i64(&meta.default, "int default"))),
			OptionType::String => OptionDefault::Value(OptionValue::String(meta.default.clone())),
		};

		let validator = meta.validator.as_deref().map(|name| {
			validator_map
				.get(name)
				.map(|v| v.validator)
				.unwrap_or_else(|| panic!("Option '{}' references unknown validator '{}'", meta.common.name, name))
		});

		defs.push(LinkedDef {
			meta: crate::defs::link::linked_meta_from_spec(&meta.common),
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

fn parse_option_type(s: &str) -> OptionType {
	match s {
		"bool" => OptionType::Bool,
		"int" => OptionType::Int,
		"string" => OptionType::String,
		other => panic!("unknown option value-type: {}", other),
	}
}

fn parse_option_scope(s: &str) -> OptionScope {
	match s {
		"global" => OptionScope::Global,
		"buffer" => OptionScope::Buffer,
		other => panic!("unknown option scope: {}", other),
	}
}

fn parse_boolish(s: &str) -> bool {
	match s {
		"#true" | "true" => true,
		"#false" | "false" => false,
		other => panic!("unknown boolean value: {}", other),
	}
}

fn parse_i64(s: &str, field: &'static str) -> i64 {
	s.parse::<i64>().unwrap_or_else(|_| panic!("invalid {field}: '{s}'"))
}
