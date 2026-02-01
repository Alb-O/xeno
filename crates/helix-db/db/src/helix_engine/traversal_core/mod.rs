//! # Traversal Core
//!
//! ## Purpose
//! High-level graph traversal engine. Implements the core logic for executing queries, filtering results, and performing mutations across the graph.
//!
//! ## Mental model
//! Traversal is modeled as an iterator-based pipeline. Each step in a query (e.g., `in_e`, `to_v`, `filter`) is an adapter that consumes an iterator of `TraversalValue` and produces a new one.
//!
//! ## Key types
//! | Type | Description |
//! | --- | --- |
//! | `HelixGraphEngine` | Primary interface for executing queries. |
//! | `TraversalValue` | Enum representing a Node, Edge, or primitive value during traversal. |
//! | `RwTraversalIterator` | Specialized iterator for write-enabled traversals. |
//!
//! ## Invariants
//! - Mutation steps must fail-fast on database errors.
//!   - Enforced in: `UpdateAdapter::update`, `AddEAdapter::add_e`, etc.
//!   - Tested by: `TODO (add regression: test_atomic_traversal_failure)`.
//!   - Failure symptom: Partial updates persist in the database, leading to inconsistency.
//! - Value filters MUST propagate `ValueError` instead of panicking or coercing to `false`.
//!   - Enforced in: `RoTraversalIterator::map_value_or`, `RwTraversalIterator::map_value_or`.
//!   - Tested by: `helix_engine::tests::traversal_tests::util_tests::test_map_value_or_propagates_value_error`.
//!   - Failure symptom: Filters silently drop values or panic on non-primitive inputs.
//!
//! ## Data flow
//! 1. Query initiated via `HelixGraphEngine`.
//! 2. Source step (e.g., `V()`) fetches initial nodes.
//! 3. Pipeline adapters process items lazily.
//! 4. Terminal step (e.g., `toList()`) collects results.
//!
//! ## Lifecycle
//! - `HelixGraphEngine` is typically wrapped in an `Arc` for shared access.
//! - Iterators are scoped to a specific transaction.
//!
//! ## Concurrency & ordering
//! - Read traversals use `RoTxn` and can run in parallel.
//! - Write traversals use `RwTxn` and are serialized by LMDB.
//!
//! ## Failure modes & recovery
//! - Query timeout: Long-running traversals may be interrupted (if supported by future async integration).
//!
//! ## Recipes
//! - Adding a traversal step: Implement a new trait/adapter in `ops/` and add it to the relevant iterator type.
//!

pub mod config;
pub mod ops;
pub mod traversal_iter;
pub mod traversal_value;

use std::sync::Arc;
#[cfg(feature = "server")]
use std::sync::Mutex;

use crate::helix_engine::storage_core::HelixGraphStorage;
use crate::helix_engine::storage_core::version_info::VersionInfo;
use crate::helix_engine::traversal_core::config::Config;
use crate::helix_engine::types::EngineError;
#[cfg(feature = "server")]
use crate::helix_gateway::mcp::mcp::{McpBackend, McpConnections};

pub const LMDB_STRING_HEADER_LENGTH: usize = 8;

#[derive(Debug)]
pub enum QueryInput {
	StringValue { value: String },
	IntegerValue { value: i32 },
	FloatValue { value: f64 },
	BooleanValue { value: bool },
}

pub struct HelixGraphEngine {
	pub storage: Arc<HelixGraphStorage>,
	#[cfg(feature = "server")]
	pub mcp_backend: Option<Arc<McpBackend>>,
	#[cfg(feature = "server")]
	pub mcp_connections: Option<Arc<Mutex<McpConnections>>>,
}

#[derive(Default, Clone)]
pub struct HelixGraphEngineOpts {
	pub path: String,
	pub config: Config,
	pub version_info: VersionInfo,
}

impl HelixGraphEngine {
	pub fn new(opts: HelixGraphEngineOpts) -> Result<HelixGraphEngine, EngineError> {
		let storage = match HelixGraphStorage::new(
			opts.path.as_str(),
			opts.config.clone(),
			opts.version_info,
		) {
			Ok(db) => Arc::new(db),
			Err(err) => return Err(err),
		};

		#[cfg(feature = "server")]
		{
			let should_use_mcp = opts.config.mcp;
			let (mcp_backend, mcp_connections) = if should_use_mcp.unwrap_or(false) {
				let mcp_backend = Arc::new(McpBackend::new(storage.clone()));
				let mcp_connections = Arc::new(Mutex::new(McpConnections::new()));
				(Some(mcp_backend), Some(mcp_connections))
			} else {
				(None, None)
			};

			Ok(Self {
				storage,
				mcp_backend,
				mcp_connections,
			})
		}

		#[cfg(not(feature = "server"))]
		{
			Ok(Self { storage })
		}
	}
}
