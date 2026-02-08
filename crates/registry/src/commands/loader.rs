use super::spec::CommandsSpec;

pub fn load_commands_spec() -> CommandsSpec {
	const BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/commands.bin"));
	crate::defs::loader::load_blob(BYTES, "commands")
}
