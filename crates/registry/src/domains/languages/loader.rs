use super::spec::LanguagesSpec;

pub fn load_languages_spec() -> LanguagesSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/languages.bin"));
	crate::defs::loader::load_blob(BYTES, "languages")
}
