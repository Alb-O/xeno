use super::spec::GrammarsSpec;

pub fn load_grammars_spec() -> GrammarsSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/grammars.bin"));
	crate::defs::loader::load_blob(BYTES, "grammars")
}
