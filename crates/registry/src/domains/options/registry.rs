//! Options registry aliases.

use crate::core::{OptionId, RegistryRef, RuntimeRegistry};
use crate::options::OptionEntry;

pub type OptionsRef = RegistryRef<OptionEntry, OptionId>;
pub type OptionsRegistry = RuntimeRegistry<OptionEntry, OptionId>;
