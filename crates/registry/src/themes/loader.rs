use super::spec::ThemesSpec;

pub fn load_themes_spec() -> ThemesSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/themes.bin"));
	crate::defs::loader::load_blob(BYTES, "themes")
}
