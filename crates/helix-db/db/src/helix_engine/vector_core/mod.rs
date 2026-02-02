//! # Vector Core
//!
//! ## Purpose
//! High-performance vector similarity search using the HNSW (Hierarchical Navigable Small World) algorithm.
//!
//! ## Mental model
//! Vectors are stored in layers of graphs. Search starts at the top layer and "zooms in" to the target vector in lower layers.
//! Metadata is stored in a dedicated properties database to decouple search logic from persistence details.
//!
//! ## Key types
//! | Type | Description |
//! | --- | --- |
//! | `VectorCore` | Manages the HNSW graph and vector persistence. |
//! | `HVector` | In-memory representation of a vector with its metadata and data. |
//! | `VectorWithoutData` | Lightweight metadata-only representation used for persistence. |
//!
//! ## Invariants
//! - Vector dimension must match index dimension.
//!   - Enforced in: `VectorCore::insert`, `HNSW::search`.
//!   - Tested by: `vector_core::tests::test_reject_dimension_mismatch`.
//!   - Failure symptom: Search returns garbage or panics in distance calculation.
//! - Vector IDs map 1:1 to nodes.
//!   - Enforced in: `VectorCore` write paths.
//!   - Tested by: `vector_core::tests::test_id_not_reused_after_delete`.
//!   - Failure symptom: Nearest-neighbor returns wrong node IDs.
//! - Persisted keys use architecture-independent sizes.
//!   - Enforced in: `VectorCore::vector_key`, `VectorCore::out_edges_key`.
//!   - Tested by: `vector_core::tests::test_portable_key_lengths`.
//!   - Failure symptom: Database cannot be opened on different architectures.
//!
//! ## Data flow
//! 1. Vector inserted via `VectorCore::insert`.
//! 2. Random level assigned to vector.
//! 3. Vector and its metadata (as `VectorWithoutData`) are persisted to LMDB.
//! 4. Search finds nearest neighbors by traversing layers.
//!
//! ## Lifecycle
//! - `VectorCore::new` opens the necessary LMDB databases.
//! - Deletion is logical (marked as deleted in metadata) but links remain for graph connectivity.
//!
//! ## Concurrency & ordering
//! - Relies on the caller to provide appropriate LMDB transactions (`RoTxn`, `RwTxn`).
//!
//! ## Failure modes & recovery
//! - Inconsistent levels: `verify_vectors_and_repair` in migration logic fixes missing level-0 entries.
//!
//! ## Recipes
//! - Performing a search: Use `HNSW::search` with a query slice and target `k`.
//!

pub mod binary_heap;
pub mod hnsw;
pub mod utils;
pub mod vector;
pub mod vector_core;
pub mod vector_distance;
pub mod vector_without_data;

#[cfg(test)]
mod tests;
