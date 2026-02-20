use super::spec::SnippetsSpec;

pub fn load_snippets_spec() -> SnippetsSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/snippets.bin"));
	crate::defs::loader::load_blob(BYTES, "snippets")
}
