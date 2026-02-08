use super::spec::MotionsSpec;

pub fn load_motions_spec() -> MotionsSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/motions.bin"));
	crate::defs::loader::load_blob(BYTES, "motions")
}
