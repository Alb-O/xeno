//! Built-in text object implementations.

pub mod brackets;
pub mod quotes;
pub mod word;

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_compiled_text_objects();
}

fn register_builtins_reg(
	builder: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), crate::db::builder::RegistryError> {
	register_builtins(builder);
	Ok(())
}

inventory::submit!(crate::db::builtins::BuiltinsReg {
	ordinal: 40,
	f: register_builtins_reg,
});
