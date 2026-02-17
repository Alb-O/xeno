use xeno_primitives::{Key, MouseEvent};

use crate::types::{Invocation, InvocationPolicy};

/// Typed input command envelope submitted by runtime event dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputDispatchCmd {
	Key(Key),
	Mouse(MouseEvent),
	Paste(String),
	Resize { cols: u16, rows: u16 },
	FocusIn,
	FocusOut,
}

/// Typed input dispatch event envelope produced by the input subsystem.
#[derive(Debug, Clone)]
pub enum InputDispatchEvt {
	InvocationRequested { invocation: Invocation, policy: InvocationPolicy },
	LocalEffectRequested(InputLocalEffect),
	OverlayCommitDeferred,
	LayoutActionRequested(LayoutActionRequest),
	FocusSyncRequested,
	Consumed,
	Unhandled,
}

/// Typed local runtime action requested by input dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputLocalEffect {
	DispatchKey(Key),
	DispatchMouse(MouseEvent),
	ApplyPaste(String),
	ApplyResize { cols: u16, rows: u16 },
	ApplyFocusIn,
	ApplyFocusOut,
}

/// Typed layout/runtime bridge request emitted by input dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutActionRequest {
	InteractionBufferEdited,
}
