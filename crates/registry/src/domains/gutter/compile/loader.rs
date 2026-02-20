use super::spec::GuttersSpec;

pub fn load_gutters_spec() -> GuttersSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/gutters.bin"));
	crate::defs::loader::load_blob(BYTES, "gutters")
}
