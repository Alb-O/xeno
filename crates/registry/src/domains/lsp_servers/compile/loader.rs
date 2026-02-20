use super::spec::LspServersSpec;

pub fn load_lsp_servers_spec() -> LspServersSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lsp_servers.bin"));
	crate::defs::loader::load_blob(BYTES, "lsp_servers")
}
