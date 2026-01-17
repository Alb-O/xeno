use crate::builder::{RegistryBuilder, RegistryError};
use crate::{MOTIONS, TEXT_OBJECTS};

pub fn ingest_all(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
	ingest_actions(builder)?;
	ingest_commands(builder)?;
	ingest_motions(builder)?;
	ingest_text_objects(builder)?;
	Ok(())
}

pub fn ingest_actions(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
	for def in crate::actions::all_actions() {
		builder.register_action(def)?;
	}
	Ok(())
}

pub fn ingest_commands(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
	for def in crate::commands::all_commands() {
		builder.register_command(def)?;
	}
	Ok(())
}

pub fn ingest_motions(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
	for &def in MOTIONS.iter() {
		builder.register_motion(def)?;
	}
	Ok(())
}

pub fn ingest_text_objects(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
	for &def in TEXT_OBJECTS.iter() {
		builder.register_text_object(def)?;
	}
	Ok(())
}
