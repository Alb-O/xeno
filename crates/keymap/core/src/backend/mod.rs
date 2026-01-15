//! Backend implementations for key event conversion.

#[cfg(feature = "termina")]
pub mod termina;
#[cfg(feature = "xeno-primitives")]
pub mod xeno;
