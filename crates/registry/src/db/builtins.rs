use crate::db::builder::{RegistryDbBuilder, RegistryError};

/// Static registration entry for a builtins registrar.
pub struct BuiltinsReg {
	pub ordinal: u16,
	pub f: fn(&mut RegistryDbBuilder) -> Result<(), RegistryError>,
}

inventory::collect!(BuiltinsReg);

/// Registers all built-in registry items with the provided builder.
pub fn register_all(builder: &mut RegistryDbBuilder) -> Result<(), RegistryError> {
	let mut regs: Vec<&'static BuiltinsReg> = inventory::iter::<BuiltinsReg>.into_iter().collect();
	regs.sort_by_key(|reg| reg.ordinal);
	for pair in regs.windows(2) {
		if pair[0].ordinal == pair[1].ordinal {
			panic!("duplicate builtins ordinal: {}", pair[0].ordinal);
		}
	}
	for reg in regs {
		(reg.f)(builder)?;
	}
	Ok(())
}
