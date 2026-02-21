//! End-to-end syntax manager tests for lifecycle and projection behavior.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::time::sleep;
use xeno_language::LanguageLoader;
use xeno_language::syntax::{InjectionPolicy, Syntax, SyntaxOptions};
use xeno_primitives::{Change, DocumentId, Rope, Transaction};

use super::invariants::{EngineGuard, MockEngine};
use super::*;

mod cache_retention;
mod lifecycle;
mod projection_selection;
mod stability;
mod viewport_lanes;
