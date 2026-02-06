use crate::core::{OptionDefault, OptionType, RegistryMeta, RegistrySource};
use crate::options::OptionScope;

fn def_bool() -> bool {
	true
}

static BAD_OPT: crate::options::OptionDef = crate::options::OptionDef {
	meta: RegistryMeta {
		id: "test::BAD_OPT",
		name: "BAD_OPT",
		aliases: &[],
		description: "bad opt",
		priority: 0,
		source: RegistrySource::Builtin,
		required_caps: &[],
		flags: 0,
	},
	kdl_key: "bad-opt",
	value_type: OptionType::Int,            // claims int
	default: OptionDefault::Bool(def_bool), // actually bool
	scope: OptionScope::Global,
	validator: None,
};

#[test]
#[should_panic(expected = "OptionDef default type mismatch")]
fn register_option_panics_on_default_type_mismatch() {
	// test the invariant directly; no builder construction needed
	super::validate_option_def(&BAD_OPT);
}
