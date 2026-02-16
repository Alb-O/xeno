//! Smith-Waterman scoring backends.
//!
//! Contains reference and SIMD implementations used by matcher kernels.

#[cfg(test)]
pub(crate) mod debug;
pub mod greedy;
pub mod reference;
pub mod simd;
