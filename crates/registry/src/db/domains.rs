//! Domain implementations for RegistryDb.

use crate::actions::def::ActionInput;
use crate::actions::entry::ActionEntry;
use crate::commands::CommandEntry;
use crate::commands::def::CommandInput;
use crate::core::index::RegistryBuilder;
use crate::core::{
	ActionId, CommandId, GutterId, HookId, MotionId, OptionId, StatuslineId, TextObjectId, ThemeId,
};
use crate::db::builder::RegistryDbBuilder;
use crate::db::domain::DomainSpec;
use crate::gutter::{GutterEntry, GutterInput};
use crate::hooks::{HookEntry, HookInput};
use crate::motions::{MotionEntry, MotionInput};
use crate::options::{OptionEntry, OptionInput};
use crate::statusline::{StatuslineEntry, StatuslineInput};
use crate::textobj::{TextObjectEntry, TextObjectInput};
use crate::themes::theme::{ThemeEntry, ThemeInput};

macro_rules! domain {
	($name:ident, $label:literal, $field:ident, $Input:ty, $Entry:ty, $Id:ty) => {
		pub struct $name;
		impl DomainSpec for $name {
			type Input = $Input;
			type Entry = $Entry;
			type Id = $Id;
			const LABEL: &'static str = $label;

			fn builder<'a>(
				db: &'a mut RegistryDbBuilder,
			) -> &'a mut RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
				&mut db.$field
			}
		}
	};
}

pub struct Actions;
impl DomainSpec for Actions {
	type Input = ActionInput;
	type Entry = ActionEntry;
	type Id = ActionId;
	const LABEL: &'static str = "actions";

	fn builder<'a>(
		db: &'a mut RegistryDbBuilder,
	) -> &'a mut RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.actions
	}

	fn on_push(db: &mut RegistryDbBuilder, input: &Self::Input) {
		match input {
			ActionInput::Static(def) => db.keybindings.extend(def.bindings.iter().cloned()),
			ActionInput::Linked(def) => db.keybindings.extend(def.bindings.iter().cloned()),
		}
	}
}

pub struct Options;
impl DomainSpec for Options {
	type Input = OptionInput;
	type Entry = OptionEntry;
	type Id = OptionId;
	const LABEL: &'static str = "options";

	fn builder<'a>(
		db: &'a mut RegistryDbBuilder,
	) -> &'a mut RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.options
	}

	fn on_push(_db: &mut RegistryDbBuilder, input: &Self::Input) {
		if let OptionInput::Static(def) = input {
			// Validate static option defaults at registration time
			if def.default.value_type() != def.value_type {
				panic!(
					"OptionDef default type mismatch: name={} kdl_key={} value_type={:?} default_type={:?}",
					def.meta.name,
					def.kdl_key,
					def.value_type,
					def.default.value_type(),
				);
			}
		}
	}
}

domain!(
	Commands,
	"commands",
	commands,
	CommandInput,
	CommandEntry,
	CommandId
);
domain!(
	Motions,
	"motions",
	motions,
	MotionInput,
	MotionEntry,
	MotionId
);
domain!(
	TextObjects,
	"text_objects",
	text_objects,
	TextObjectInput,
	TextObjectEntry,
	TextObjectId
);
domain!(Themes, "themes", themes, ThemeInput, ThemeEntry, ThemeId);
domain!(
	Gutters,
	"gutters",
	gutters,
	GutterInput,
	GutterEntry,
	GutterId
);
domain!(
	Statusline,
	"statusline",
	statusline,
	StatuslineInput,
	StatuslineEntry,
	StatuslineId
);
domain!(Hooks, "hooks", hooks, HookInput, HookEntry, HookId);
