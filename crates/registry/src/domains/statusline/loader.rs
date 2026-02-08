use super::spec::StatuslineSpec;

pub fn load_statusline_spec() -> StatuslineSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/statusline.bin"));
	crate::defs::loader::load_blob(BYTES, "statusline")
}
