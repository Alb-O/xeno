// Copyright 2025 HelixDB Inc.
// SPDX-License-Identifier: AGPL-3.0

//! # Helix Compiler (helixc)
//!
//! ## Purpose
//! The compiler for HelixQL. It parses, analyzes, and transpiles HelixDB’s query
//! language into optimized Rust code that can be embedded into the database engine.
//!
//! ## Mental model
//! The compiler is a traditional multi-stage system:
//! 1. Parser (Pest): Converts source to an Untyped AST.
//! 2. Analyzer: Performs semantic validation and type inference.
//! 3. Generator: Produces Rust code from the Typed AST.
//!
//! ## Key types
//! | Type | Description |
//! | --- | --- |
//! | `Source` | The primary input representing a complete HelixQL schema/query set. |
//! | `Query` | Representation of a single HelixQL query. |
//! | `Analyzer` | Context for semantic validation and type checking. |
//!
//! ## Invariants
//! - Generated Rust code must be valid and hygiene-friendly.
//!   - Enforced in: `Generator` implementations.
//!   - Tested by: `TODO (add regression: test_generated_code_builds)`.
//!   - Failure symptom: Rust compiler errors in the generated `queries.rs`.
//! - Type inference must resolve all variables before generation.
//!   - Enforced in: `Analyzer::infer_expr_type`.
//!   - Tested by: `analyzer::tests::*`.
//!   - Failure symptom: Generator panics or emits `unimplemented!`.
//!
//! ## Data flow
//! 1. HelixQL string input.
//! 2. `parser` generates AST nodes.
//! 3. `analyzer` validates schemas and infers types.
//! 4. `generator` emits Rust source code.
//!
//! ## Lifecycle
//! - Typically used as a build-time tool or a standalone CLI.
//!
//! ## Concurrency & ordering
//! - The compilation pipeline is largely sequential per source file.
//!
//! ## Failure modes & recovery
//! - Syntax error: Reported with localized diagnostics via `ariadne`.
//! - Semantic error: Reported during analysis pass.
//!
//! ## Recipes
//! - Compiling a file: Use `helixc::generate` with a `Source` object.

pub mod analyzer;
pub mod generator;
pub mod parser;
