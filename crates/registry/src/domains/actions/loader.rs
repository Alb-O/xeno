use super::spec::ActionsSpec;

pub fn load_actions_spec() -> ActionsSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/actions.bin"));
	crate::defs::loader::load_blob(BYTES, "actions")
}
