//! Buffer - the core text editing unit.
//! Anchor ID: XENO_ANCHOR_BUFFER
//!
//! # Purpose
//!
//! * Owns: per-view state (cursor, selection, scroll position, local options) and modal input state.
//! * Does not own: authoritative document content (owned by [`crate::core::document::Document`]).
//! * Source of truth: [`crate::buffer::Buffer`].
//!
//! # Mental model
//!
//! * A buffer is a view into a document.
//! * Multiple buffers can point to the same document (enabling splits).
//! * View-local state (like the cursor) is stored in the buffer.
//! * Shared state (like text and history) is stored in the document.
//! * Thread-safety for shared documents is managed via `DocumentHandle` with re-entrancy protection.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`crate::buffer::Buffer`] | Primary editing unit | Must separate view state from content | `Buffer::new`, `Buffer::clone_for_split` |
//! | [`crate::core::document::Document`] | Shared content | Authoritative source of text/history | `Document::new` |
//! | `DocumentHandle` | Thread-safe wrapper | Must prevent re-entrant locks on same thread | `DocumentHandle::new` |
//! | [`crate::buffer::ApplyPolicy`] | Edit validation rules | Controls readonly/history behavior | `editing::apply` |
//!
//! # Invariants
//!
//! * Must not allow re-entrant locking of the same document on a single thread.
//! * Must keep view state (cursor/selection) within document bounds.
//! * Must preserve monotonic document versions across edits.
//!
//! # Data flow
//!
//! 1. Input: User keys flow into [`InputHandler`].
//! 2. Resolution: Input produces an action which calls `Buffer` methods.
//! 3. Mutation: `Buffer` calls `DocumentHandle::with_mut` to apply edits.
//! 4. Notification: Document changes trigger version bumps and event emission.
//!
//! # Lifecycle
//!
//! 1. Creation: `Buffer::new` creates a view over a fresh [`crate::core::document::Document`].
//! 2. Split: `Buffer::clone_for_split` creates additional views over the same document.
//! 3. Editing: input handlers mutate document state through `DocumentHandle`.
//! 4. Disposal: dropping a buffer releases only view-local state; document lifetime follows shared ownership.
//!
//! # Concurrency & ordering
//!
//! * Multi-view consistency: Edits to a shared document are immediately visible to all buffers.
//! * Lock ordering: Always acquire document locks for the shortest possible duration.
//! * Thread-safety: `Document` is wrapped in `Arc<RwLock<Document>>` inside `DocumentHandle`.
//!
//! # Failure modes & recovery
//!
//! * Readonly violation: Edits to readonly documents/buffers return `EditError`.
//! * Deadlock prevention: Re-entrant lock attempts trigger a controlled panic via `LockGuard`.
//!
//! # Recipes
//!
//! ## Split a view
//!
//! * Call `buffer.clone_for_split(new_view_id)`.
//! * This creates a new buffer sharing the same `DocumentHandle`.
//!
//! ## Apply an edit
//!
//! * Use `buffer.apply(&tx, policy)`.
//! * This handles versioning, history, and readonly checks automatically.
//!
mod editing;

mod layout;
mod navigation;
mod state;

#[cfg(test)]
mod invariants;

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub use editing::ApplyPolicy;
pub use layout::{Layout, SpatialDirection, SplitDirection, SplitPath};
use parking_lot::RwLock;
pub use state::Buffer;
pub(crate) use state::CommitBypassToken;
#[cfg(test)]
pub(crate) use state::LockGuard;
use xeno_input::input::InputHandler;
use xeno_language::LanguageLoader;
pub use xeno_primitives::ViewId;
use xeno_primitives::{CharIdx, Mode, Selection};
use xeno_registry::options::{FromOptionValue, OptionKey, OptionStore, OptionValue, TypedOptionKey};

pub use crate::core::document::{Document, DocumentId, DocumentMetaOutcome};
pub use crate::core::history::HistoryResult;
pub use crate::core::undo_store::{TxnUndoStore, UndoBackend};
