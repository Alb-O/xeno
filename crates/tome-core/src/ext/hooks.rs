//! Hook system for editor lifecycle events.
//!
//! Hooks allow extensions to react to editor events like file open, save,
//! mode change, etc. They are registered at compile-time using `linkme`
//! and executed when the corresponding event occurs.
//!
//! # Example
//!
//! ```ignore
//! use tome_core::ext::{HookDef, HookEvent, HookContext, HOOKS};
//! use linkme::distributed_slice;
//!
//! #[distributed_slice(HOOKS)]
//! static FORMAT_ON_SAVE: HookDef = HookDef {
//!     name: "format_on_save",
//!     event: HookEvent::BufferWrite,
//!     description: "Format buffer before saving",
//!     handler: |ctx| {
//!         if let HookContext::BufferWrite { path, .. } = ctx {
//!             // format the file
//!         }
//!     },
//! };
//! ```

use linkme::distributed_slice;
use ropey::RopeSlice;
use std::path::Path;

use crate::Mode;

/// Events that can trigger hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    /// Editor is starting up (before first render).
    EditorStart,
    /// Editor is shutting down.
    EditorQuit,

    /// A buffer was opened/created.
    BufferOpen,
    /// A buffer is about to be written to disk.
    BufferWritePre,
    /// A buffer was written to disk.
    BufferWrite,
    /// A buffer was closed.
    BufferClose,
    /// Buffer content changed.
    BufferChange,

    /// Mode changed (normal -> insert, etc).
    ModeChange,

    /// Cursor position changed.
    CursorMove,
    /// Selection changed.
    SelectionChange,

    /// Window was resized.
    WindowResize,
    /// Window gained focus.
    FocusGained,
    /// Window lost focus.
    FocusLost,
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::EditorStart => "editor:start",
            HookEvent::EditorQuit => "editor:quit",
            HookEvent::BufferOpen => "buffer:open",
            HookEvent::BufferWritePre => "buffer:write-pre",
            HookEvent::BufferWrite => "buffer:write",
            HookEvent::BufferClose => "buffer:close",
            HookEvent::BufferChange => "buffer:change",
            HookEvent::ModeChange => "mode:change",
            HookEvent::CursorMove => "cursor:move",
            HookEvent::SelectionChange => "selection:change",
            HookEvent::WindowResize => "window:resize",
            HookEvent::FocusGained => "focus:gained",
            HookEvent::FocusLost => "focus:lost",
        }
    }
}

/// Context passed to hook handlers.
///
/// Each event variant contains data relevant to that event.
/// Handlers should match on the expected variant.
pub enum HookContext<'a> {
    /// Editor startup context.
    EditorStart,

    /// Editor quit context.
    EditorQuit,

    /// Buffer was opened.
    BufferOpen {
        path: &'a Path,
        text: RopeSlice<'a>,
        file_type: Option<&'a str>,
    },

    /// Buffer is about to be written.
    BufferWritePre {
        path: &'a Path,
        text: RopeSlice<'a>,
    },

    /// Buffer was written.
    BufferWrite {
        path: &'a Path,
    },

    /// Buffer was closed.
    BufferClose {
        path: &'a Path,
    },

    /// Buffer content changed.
    BufferChange {
        path: &'a Path,
        text: RopeSlice<'a>,
    },

    /// Mode changed.
    ModeChange {
        old_mode: Mode,
        new_mode: Mode,
    },

    /// Cursor moved.
    CursorMove {
        line: usize,
        col: usize,
    },

    /// Selection changed.
    SelectionChange {
        anchor: usize,
        head: usize,
    },

    /// Window resized.
    WindowResize {
        width: u16,
        height: u16,
    },

    /// Window focus gained.
    FocusGained,

    /// Window focus lost.
    FocusLost,
}

impl<'a> HookContext<'a> {
    pub fn event(&self) -> HookEvent {
        match self {
            HookContext::EditorStart => HookEvent::EditorStart,
            HookContext::EditorQuit => HookEvent::EditorQuit,
            HookContext::BufferOpen { .. } => HookEvent::BufferOpen,
            HookContext::BufferWritePre { .. } => HookEvent::BufferWritePre,
            HookContext::BufferWrite { .. } => HookEvent::BufferWrite,
            HookContext::BufferClose { .. } => HookEvent::BufferClose,
            HookContext::BufferChange { .. } => HookEvent::BufferChange,
            HookContext::ModeChange { .. } => HookEvent::ModeChange,
            HookContext::CursorMove { .. } => HookEvent::CursorMove,
            HookContext::SelectionChange { .. } => HookEvent::SelectionChange,
            HookContext::WindowResize { .. } => HookEvent::WindowResize,
            HookContext::FocusGained => HookEvent::FocusGained,
            HookContext::FocusLost => HookEvent::FocusLost,
        }
    }
}

/// A hook that responds to editor events.
#[derive(Clone, Copy)]
pub struct HookDef {
    /// Hook name for debugging/logging.
    pub name: &'static str,
    /// The event this hook responds to.
    pub event: HookEvent,
    /// Short description.
    pub description: &'static str,
    /// Priority (lower runs first, default 100).
    pub priority: i32,
    /// The hook handler function.
    pub handler: fn(&HookContext),
}

impl std::fmt::Debug for HookDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookDef")
            .field("name", &self.name)
            .field("event", &self.event)
            .field("priority", &self.priority)
            .field("description", &self.description)
            .finish()
    }
}

/// Registry of all hook definitions.
#[distributed_slice]
pub static HOOKS: [HookDef];

/// Emit an event to all registered hooks.
///
/// Hooks are called in priority order (lower priority first).
/// All hooks matching the event will be called.
pub fn emit(ctx: &HookContext) {
    let event = ctx.event();

    let mut matching: Vec<_> = HOOKS.iter().filter(|h| h.event == event).collect();
    matching.sort_by_key(|h| h.priority);

    for hook in matching {
        (hook.handler)(ctx);
    }
}

/// Find all hooks registered for a specific event.
pub fn find_hooks(event: HookEvent) -> impl Iterator<Item = &'static HookDef> {
    HOOKS.iter().filter(move |h| h.event == event)
}

/// List all registered hooks.
pub fn all_hooks() -> &'static [HookDef] {
    &HOOKS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_as_str() {
        assert_eq!(HookEvent::BufferWrite.as_str(), "buffer:write");
        assert_eq!(HookEvent::ModeChange.as_str(), "mode:change");
    }

    #[test]
    fn test_hook_context_event() {
        let ctx = HookContext::ModeChange {
            old_mode: Mode::Normal,
            new_mode: Mode::Insert,
        };
        assert_eq!(ctx.event(), HookEvent::ModeChange);
    }
}
