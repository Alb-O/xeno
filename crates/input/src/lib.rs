pub mod handler;
pub mod insert;
pub mod normal;
pub mod pending;
#[cfg(test)]
mod tests;
pub mod types;

// Re-export Mode from evildoer-manifest for convenience
pub use evildoer_manifest::Mode;
pub use handler::InputHandler;
pub use types::KeyResult;
