use super::spec::OptionsSpec;

pub fn load_options_spec() -> OptionsSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/options.bin"));
	crate::defs::loader::load_blob(BYTES, "options")
}
