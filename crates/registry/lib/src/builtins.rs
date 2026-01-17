use crate::builder::{RegistryBuilder, RegistryError};
use crate::plugin::XenoPlugin;
use crate::{actions, commands, motions, textobj};

pub struct ActionsPlugin;

impl XenoPlugin for ActionsPlugin {
	const ID: &'static str = "actions";

	fn register(reg: &mut RegistryBuilder) -> Result<(), RegistryError> {
		for def in actions::all_actions() {
			reg.register_action(def)?;
		}
		Ok(())
	}
}

pub struct CommandsPlugin;

impl XenoPlugin for CommandsPlugin {
	const ID: &'static str = "commands";

	fn register(reg: &mut RegistryBuilder) -> Result<(), RegistryError> {
		for def in commands::all_commands() {
			reg.register_command(def)?;
		}
		Ok(())
	}
}

pub struct MotionsPlugin;

impl XenoPlugin for MotionsPlugin {
	const ID: &'static str = "motions";

	fn register(reg: &mut RegistryBuilder) -> Result<(), RegistryError> {
		for def in motions::all() {
			reg.register_motion(def)?;
		}
		Ok(())
	}
}

pub struct TextObjectsPlugin;

impl XenoPlugin for TextObjectsPlugin {
	const ID: &'static str = "text_objects";

	fn register(reg: &mut RegistryBuilder) -> Result<(), RegistryError> {
		for def in textobj::all() {
			reg.register_text_object(def)?;
		}
		Ok(())
	}
}

/// Registers all built-in registry items with the provided builder.
pub fn register_all(reg: &mut RegistryBuilder) -> Result<(), RegistryError> {
	ActionsPlugin::register(reg)?;
	CommandsPlugin::register(reg)?;
	MotionsPlugin::register(reg)?;
	TextObjectsPlugin::register(reg)?;
	Ok(())
}
