//! Domain implementations for RegistryDb.

use crate::actions::def::ActionInput;
use crate::actions::entry::ActionEntry;
use crate::commands::CommandEntry;
use crate::commands::def::CommandInput;
use crate::core::index::RegistryBuilder;
use crate::core::{
	ActionId, CommandId, GutterId, HookId, LanguageId, MotionId, OptionId, StatuslineId,
	TextObjectId, ThemeId,
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
	(
		$name:ident,
		$label:literal,
		$field:ident,
		$Input:ty,
		$Entry:ty,
		$Id:ty
		$(, on_push($db:ident, $input:ident) $body:block )?
	) => {
		pub struct $name;

		impl DomainSpec for $name {
			type Input = $Input;
			type Entry = $Entry;
			type Id = $Id;
			const LABEL: &'static str = $label;

			fn builder(
				db: &mut RegistryDbBuilder,
			) -> &mut RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
				&mut db.$field
			}

			$(
				fn on_push($db: &mut RegistryDbBuilder, $input: &Self::Input) $body
			)?
		}
	};
}

domain!(
	Actions,
	"actions",
	actions,
	ActionInput,
	ActionEntry,
	ActionId,
	on_push(db, input) {
		match input {
			ActionInput::Static(def) => db.keybindings.extend(def.bindings.iter().cloned()),
			ActionInput::Linked(def) => db.keybindings.extend(def.payload.bindings.iter().cloned()),
		}
	}
);

domain!(
	Options,
	"options",
	options,
	OptionInput,
	OptionEntry,
	OptionId,
	on_push(_db, input) {
		if let OptionInput::Static(def) = input {
			crate::db::builder::validate_option_def(def);
		}
	}
);

domain!(
	Notifications,
	"notifications",
	notifications,
	crate::notifications::NotificationInput,
	crate::notifications::NotificationEntry,
	crate::notifications::NotificationId
);

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
domain!(
	Languages,
	"languages",
	languages,
	crate::languages::LanguageInput,
	crate::languages::LanguageEntry,
	LanguageId
);
