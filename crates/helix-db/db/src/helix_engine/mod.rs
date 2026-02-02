//! # Helix Engine
//!
//! ## Purpose
//! The core database engine of HelixDB. It integrates the storage core (graph structure),
//! vector core (similarity search), and BM25 indexing into a unified system for
//! hybrid graph-vector queries.
//!
//! ## Mental model
//! The engine acts as a coordinator. It manages the lifecycle of the underlying LMDB
//! environment and provides a high-level API for executing traversals and mutations.
//!
//! ## Key types
//! | Type | Description |
//! | --- | --- |
//! | `HelixGraphEngine` | The primary handle for the database engine. |
//! | `HelixGraphStorage` | The underlying storage layer. |
//!
//! ## Invariants
//! - Subsystem initialization must follow a strict order (Storage -> Vector -> BM25).
//!   - Enforced in: `HelixGraphEngine::new`.
//!   - Tested by: `TODO (add regression: test_engine_init_order)`.
//!   - Failure symptom: Subsystems fail to find their databases or metadata.
//!
//! ## Data flow
//! 1. Query request received (e.g., from Gateway).
//! 2. Engine creates a transaction (Read or Write).
//! 3. Traversal logic interacts with Storage, Vector, and BM25 cores.
//! 4. Results are collected and returned.
//!
//! ## Lifecycle
//! - `HelixGraphEngine::new` initializes the entire stack.
//! - `HelixGraphEngine` is typically wrapped in an `Arc` for shared access.
//!
//! ## Concurrency & ordering
//! - Thread-safe via `Arc`.
//! - Read operations are highly concurrent; writers are serialized by LMDB.
//!
//! ## Failure modes & recovery
//! - Transaction abort: Rollback is handled automatically by LMDB.
//!
//! ## Recipes
//! - Starting the engine: Use `HelixGraphEngine::new` with `HelixGraphEngineOpts`.

pub mod bm25;
pub mod macros;
pub mod reranker;
pub mod storage_core;
pub mod traversal_core;
pub mod types;
pub mod vector_core;

#[cfg(test)]
mod tests;
