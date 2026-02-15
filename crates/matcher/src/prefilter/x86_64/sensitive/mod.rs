//! x86_64-optimized case-sensitive prefilter variants.

mod ordered;
mod unordered;
mod unordered_typos;

pub use ordered::*;
pub use unordered::*;
pub use unordered_typos::*;
