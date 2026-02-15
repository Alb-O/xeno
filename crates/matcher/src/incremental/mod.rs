//! Incremental matcher subsystem.
//!
//! Maintains bucketed match state for low-latency updates as needle or haystack
//! sets change incrementally.

mod bucket;
mod bucket_collection;
mod matcher;

pub use matcher::IncrementalMatcher;
