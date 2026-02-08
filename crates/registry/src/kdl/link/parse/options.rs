use crate::core::{OptionType, OptionValue};
use crate::options::def::OptionScope;

pub(crate) fn parse_option_type(s: &str) -> OptionType {
	match s {
		"bool" => OptionType::Bool,
		"int" => OptionType::Int,
		"string" => OptionType::String,
		other => super::unknown("option value-type", other),
	}
}

pub(crate) fn parse_option_scope(s: &str) -> OptionScope {
	match s {
		"global" => OptionScope::Global,
		"buffer" => OptionScope::Buffer,
		other => super::unknown("option scope", other),
	}
}

pub(crate) fn parse_boolish(s: &str) -> bool {
	match s {
		"#true" | "true" => true,
		"#false" | "false" => false,
		other => super::unknown("boolean value", other),
	}
}

pub(crate) fn parse_i64(s: &str, field: &'static str) -> i64 {
	s.parse::<i64>()
		.unwrap_or_else(|_| panic!("invalid {field}: '{s}'"))
}

pub(crate) fn parse_option_value(s: &str, ty: OptionType) -> OptionValue {
	match ty {
		OptionType::Bool => OptionValue::Bool(parse_boolish(s)),
		OptionType::Int => OptionValue::Int(parse_i64(s, "int default")),
		OptionType::String => OptionValue::String(s.to_string()),
	}
}
