use super::spec::TextObjectsSpec;

pub fn load_text_objects_spec() -> TextObjectsSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/text_objects.bin"));
	crate::defs::loader::load_blob(BYTES, "text_objects")
}
