use crate::builder::{RegistryBuilder, RegistryError};

pub trait XenoPlugin {
	const ID: &'static str;

	fn register(reg: &mut RegistryBuilder) -> Result<(), RegistryError>;
}
