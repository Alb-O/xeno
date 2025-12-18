//! Mode change result handler.

use crate::ext::actions::{ActionMode, ActionResult};
use crate::ext::editor_ctx::HandleOutcome;
use crate::{Mode, result_handler};

result_handler!(
	RESULT_MODE_CHANGE_HANDLERS,
	HANDLE_MODE_CHANGE,
	"mode_change",
	|r, ctx, _| {
		if let ActionResult::ModeChange(mode) = r {
			let new_mode = match mode {
				ActionMode::Normal => Mode::Normal,
				ActionMode::Insert => Mode::Insert,
				ActionMode::Goto => Mode::Goto,
				ActionMode::View => Mode::View,
				ActionMode::Command => Mode::Command {
					prompt: ':',
					input: String::new(),
				},
				ActionMode::SearchForward => Mode::Command {
					prompt: '/',
					input: String::new(),
				},
				ActionMode::SearchBackward => Mode::Command {
					prompt: '?',
					input: String::new(),
				},
				ActionMode::SelectRegex => Mode::Command {
					prompt: 's',
					input: String::new(),
				},
				ActionMode::SplitRegex => Mode::Command {
					prompt: 'S',
					input: String::new(),
				},
				ActionMode::KeepMatching => Mode::Command {
					prompt: 'k',
					input: String::new(),
				},
				ActionMode::KeepNotMatching => Mode::Command {
					prompt: 'K',
					input: String::new(),
				},
				ActionMode::PipeReplace => Mode::Command {
					prompt: '|',
					input: String::new(),
				},
				ActionMode::PipeIgnore => Mode::Command {
					prompt: '\\',
					input: String::new(),
				},
				ActionMode::InsertOutput => Mode::Command {
					prompt: '!',
					input: String::new(),
				},
				ActionMode::AppendOutput => Mode::Command {
					prompt: '@',
					input: String::new(),
				},
			};
			ctx.set_mode(new_mode);
		}
		HandleOutcome::Handled
	}
);
