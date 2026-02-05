//! Headless core model types for Xeno.
//!
//! This module owns document state, undo backends, and core history primitives.
//! It intentionally excludes UI, LSP, and overlay concerns.

pub mod document;
pub mod history;
pub mod undo_store;

pub use document::{Document, DocumentId};
pub use history::HistoryResult;
pub use undo_store::{TxnUndoStore, UndoBackend};
