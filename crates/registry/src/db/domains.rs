use crate::actions::ActionDef;
use crate::actions::def::ActionInput;
use crate::actions::entry::ActionEntry;
use crate::commands::def::CommandInput;
use crate::commands::{CommandDef, CommandEntry};
use crate::core::index::RegistryBuilder;
use crate::core::{
	ActionId, CommandId, GutterId, HookId, LanguageId, MotionId, OptionId, StatuslineId,
	TextObjectId, ThemeId,
};
use crate::db::builder::RegistryDbBuilder;
use crate::db::domain::DomainSpec;
use crate::gutter::{GutterDef, GutterEntry, GutterInput};
use crate::hooks::{HookDef, HookEntry, HookInput};
use crate::motions::{MotionDef, MotionEntry, MotionInput};
use crate::options::{OptionDef, OptionEntry, OptionInput};
use crate::statusline::{StatuslineEntry, StatuslineInput, StatuslineSegmentDef};
use crate::symbol::LspServerId;
use crate::textobj::{TextObjectDef, TextObjectEntry, TextObjectInput};
use crate::themes::theme::{ThemeDef, ThemeEntry, ThemeInput};

macro_rules! domain {
	(
		$name:ident,
		$label:literal,
		$field:ident,
		$Input:ty,
		$Entry:ty,
		$Id:ty,
		$StaticDef:ty,
		$static_to_input:expr,
		$LinkedDef:ty,
		$linked_to_input:expr
		$(, on_push($db:ident, $input:ident) $body:block )?
	) => {
		pub struct $name;

		impl DomainSpec for $name {
			type Input = $Input;
			type Entry = $Entry;
			type Id = $Id;
			type StaticDef = $StaticDef;
			type LinkedDef = $LinkedDef;
			const LABEL: &'static str = $label;

			fn static_to_input(def: &'static Self::StaticDef) -> Self::Input {
				($static_to_input)(def)
			}

			fn linked_to_input(def: Self::LinkedDef) -> Self::Input {
				($linked_to_input)(def)
			}

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
	ActionDef,
	|def: &'static ActionDef| ActionInput::Static(def.clone()),
	crate::actions::link::LinkedActionDef,
	|def: crate::actions::link::LinkedActionDef| ActionInput::Linked(def),
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
	OptionDef,
	|def: &'static OptionDef| OptionInput::Static(def.clone()),
	crate::options::link::LinkedOptionDef,
	|def: crate::options::link::LinkedOptionDef| OptionInput::Linked(def),
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
	crate::notifications::NotificationId,
	crate::notifications::NotificationDef,
	|def: &'static crate::notifications::NotificationDef| {
		crate::notifications::NotificationInput::Static(*def)
	},
	crate::notifications::def::LinkedNotificationDef,
	|def: crate::notifications::def::LinkedNotificationDef| {
		crate::notifications::NotificationInput::Linked(def)
	}
);

domain!(
	Commands,
	"commands",
	commands,
	CommandInput,
	CommandEntry,
	CommandId,
	CommandDef,
	|def: &'static CommandDef| CommandInput::Static(def.clone()),
	crate::commands::link::LinkedCommandDef,
	|def: crate::commands::link::LinkedCommandDef| CommandInput::Linked(def)
);

domain!(
	Motions,
	"motions",
	motions,
	MotionInput,
	MotionEntry,
	MotionId,
	MotionDef,
	|def: &'static MotionDef| MotionInput::Static(def.clone()),
	crate::motions::link::LinkedMotionDef,
	|def: crate::motions::link::LinkedMotionDef| MotionInput::Linked(def)
);

domain!(
	TextObjects,
	"text_objects",
	text_objects,
	TextObjectInput,
	TextObjectEntry,
	TextObjectId,
	TextObjectDef,
	|def: &'static TextObjectDef| TextObjectInput::Static(*def),
	crate::textobj::link::LinkedTextObjectDef,
	|def: crate::textobj::link::LinkedTextObjectDef| TextObjectInput::Linked(def)
);

domain!(
	Themes,
	"themes",
	themes,
	ThemeInput,
	ThemeEntry,
	ThemeId,
	ThemeDef,
	|def: &'static ThemeDef| ThemeInput::Static(*def),
	crate::themes::theme::LinkedThemeDef,
	|def: crate::themes::theme::LinkedThemeDef| ThemeInput::Linked(def)
);

domain!(
	Gutters,
	"gutters",
	gutters,
	GutterInput,
	GutterEntry,
	GutterId,
	GutterDef,
	|def: &'static GutterDef| GutterInput::Static(*def),
	crate::gutter::link::LinkedGutterDef,
	|def: crate::gutter::link::LinkedGutterDef| GutterInput::Linked(def)
);

domain!(
	Statusline,
	"statusline",
	statusline,
	StatuslineInput,
	StatuslineEntry,
	StatuslineId,
	StatuslineSegmentDef,
	|def: &'static StatuslineSegmentDef| StatuslineInput::Static(*def),
	crate::statusline::link::LinkedStatuslineDef,
	|def: crate::statusline::link::LinkedStatuslineDef| StatuslineInput::Linked(def)
);

domain!(
	Hooks,
	"hooks",
	hooks,
	HookInput,
	HookEntry,
	HookId,
	HookDef,
	|def: &'static HookDef| HookInput::Static(*def),
	crate::hooks::link::LinkedHookDef,
	|def: crate::hooks::link::LinkedHookDef| HookInput::Linked(def)
);

domain!(
	Languages,
	"languages",
	languages,
	crate::languages::LanguageInput,
	crate::languages::LanguageEntry,
	LanguageId,
	crate::languages::types::LanguageDef,
	|def: &'static crate::languages::types::LanguageDef| crate::languages::LanguageInput::Static(
		def.clone()
	),
	crate::languages::link::LinkedLanguageDef,
	|def: crate::languages::link::LinkedLanguageDef| crate::languages::LanguageInput::Linked(def)
);

domain!(
	LspServers,
	"lsp_servers",
	lsp_servers,
	crate::lsp_servers::LspServerInput,
	crate::lsp_servers::LspServerEntry,
	LspServerId,
	crate::lsp_servers::entry::LspServerDef,
	|def: &'static crate::lsp_servers::entry::LspServerDef| {
		crate::lsp_servers::LspServerInput::Static(def.clone())
	},
	crate::core::LinkedDef<crate::lsp_servers::entry::LspServerPayload>,
	|def: crate::core::LinkedDef<crate::lsp_servers::entry::LspServerPayload>| {
		crate::lsp_servers::LspServerInput::Linked(def)
	}
);
