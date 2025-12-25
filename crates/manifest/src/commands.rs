use futures::future::LocalBoxFuture;
use ropey::RopeSlice;

use crate::{Capability, CommandError, CommandOutcome, EditorOps, RegistrySource};

pub struct CommandDef {
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub description: &'static str,
	pub handler: for<'a> fn(
		_: &'a mut CommandContext<'a>,
	) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>>,
	pub user_data: Option<&'static (dyn std::any::Any + Sync)>,
	pub priority: i16,
	pub source: RegistrySource,
	pub required_caps: &'static [Capability],
	pub flags: u32,
}

pub struct CommandContext<'a> {
	pub editor: &'a mut dyn EditorOps,
	pub args: &'a [&'a str],
	pub count: usize,
	pub register: Option<char>,
	pub user_data: Option<&'static (dyn std::any::Any + Sync)>,
}

impl<'a> CommandContext<'a> {
	pub fn text(&self) -> RopeSlice<'_> {
		self.editor.text()
	}
	pub fn message(&mut self, msg: &str) {
		self.editor.show_message(msg);
	}
	pub fn error(&mut self, msg: &str) {
		self.editor.show_error(msg);
	}
	pub fn require_user_data<T: std::any::Any + Sync>(&self) -> Result<&'static T, CommandError> {
		self.user_data
			.and_then(|d| {
				let any: &dyn std::any::Any = d;
				any.downcast_ref::<T>()
			})
			.ok_or_else(|| {
				CommandError::Other(format!(
					"Missing or invalid user data for command (expected {})",
					std::any::type_name::<T>()
				))
			})
	}
}

pub mod flags {
	pub const NONE: u32 = 0;
	pub const HIDDEN: u32 = 1 << 0;
	pub const EXPERIMENTAL: u32 = 1 << 1;
	pub const UNSAFE: u32 = 1 << 2;
}

impl crate::RegistryMetadata for CommandDef {
	fn id(&self) -> &'static str {
		self.id
	}
	fn name(&self) -> &'static str {
		self.name
	}
	fn priority(&self) -> i16 {
		self.priority
	}
	fn source(&self) -> crate::RegistrySource {
		self.source
	}
}
