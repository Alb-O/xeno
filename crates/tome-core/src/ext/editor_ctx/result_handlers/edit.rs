//! Edit action result handler.

use linkme::distributed_slice;

use crate::ext::actions::ActionResult;
use crate::ext::editor_ctx::{HandleOutcome, ResultHandler, RESULT_HANDLERS};

#[distributed_slice(RESULT_HANDLERS)]
static HANDLE_EDIT: ResultHandler = ResultHandler {
    name: "edit",
    handles: |r| matches!(r, ActionResult::Edit(_)),
    handle: |r, ctx, extend| {
        if let ActionResult::Edit(action) = r
            && let Some(edit) = ctx.edit() {
                let quit = edit.execute_edit(action, extend);
                return if quit { HandleOutcome::Quit } else { HandleOutcome::Handled };
            }
        HandleOutcome::NotHandled
    },
};
