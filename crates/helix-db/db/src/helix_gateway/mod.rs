//! # Helix Gateway
//!
//! ## Purpose
//! Provides the network interface for HelixDB, including an HTTP server and
//! Model Context Protocol (MCP) integration.
//!
//! ## Mental model
//! The gateway acts as the primary entry point for external clients. It routes incoming
//! requests to the appropriate handlers, manages a pool of workers for query execution,
//! and handles authentication and schema introspection.
//!
//! ## Key types
//! | Type | Description |
//! | --- | --- |
//! | `HelixRouter` | Manages registration and routing of request handlers. |
//! | `MCPToolInput` | Context provided to MCP tool handlers. |
//! | `WorkerPool` | Manages concurrent execution of database operations. |
//!
//! ## Invariants
//! - All routes must be registered before the server starts.
//!   - Enforced in: `HelixRouter` initialization.
//!   - Tested by: `router_tests::test_router_new_with_routes`.
//!   - Failure symptom: Clients receive 404 for valid endpoints.
//! - Write operations must be explicitly flagged and serialized.
//!   - Enforced in: `HelixRouter::is_write_route`.
//!   - Tested by: `router_tests::test_router_is_write_route_true`.
//!   - Failure symptom: Multiple concurrent writers causing transaction conflicts.
//!
//! ## Data flow
//! 1. External request received by `Axum` or MCP transport.
//! 2. `HelixRouter` identifies the target handler.
//! 3. Request dispatched to `WorkerPool`.
//! 4. Handler executes query via `HelixGraphEngine`.
//! 5. Response formatted and returned to client.
//!
//! ## Lifecycle
//! - Gateway is started via `run_gateway` or similar entry points.
//! - Handlers are registered via `inventory` at startup.
//!
//! ## Concurrency & ordering
//! - HTTP handlers run concurrently in the `Axum` runtime.
//! - Database access is mediated by the `WorkerPool` to prevent environment contention.
//!
//! ## Failure modes & recovery
//! - Network errors: Handled by the underlying transport.
//!
//! ## Recipes
//! - Adding a new endpoint: Use the `#[handler]` or `#[mcp_handler]` attributes.

#[cfg(feature = "dev-instance")]
pub mod builtin;
pub mod gateway;
pub mod introspect_schema;
#[cfg(feature = "api-key")]
pub mod key_verification;
pub mod mcp;
pub mod router;
#[cfg(test)]
pub mod tests;
pub mod worker_pool;
