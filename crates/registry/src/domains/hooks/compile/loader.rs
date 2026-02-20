use super::spec::HooksSpec;

pub fn load_hooks_spec() -> HooksSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/hooks.bin"));
	crate::defs::loader::load_blob(BYTES, "hooks")
}
