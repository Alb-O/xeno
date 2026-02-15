//! End-to-end syntax manager tests for lifecycle and projection behavior.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::time::sleep;
use xeno_language::LanguageLoader;
use xeno_language::syntax::{InjectionPolicy, Syntax, SyntaxOptions};
use xeno_primitives::transaction::Change;
use xeno_primitives::{Rope, Transaction};

use super::invariants::{EngineGuard, MockEngine};
use super::*;
use crate::core::document::DocumentId;

mod cache_retention;
mod lifecycle;
mod projection_selection;
mod stability;
mod viewport_lanes;
