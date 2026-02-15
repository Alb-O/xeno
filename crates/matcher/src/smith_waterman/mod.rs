//! Smith-Waterman scoring backends.
//!
//! Contains reference and SIMD implementations used by matcher kernels.

pub mod greedy;
pub mod reference;
pub mod simd;
