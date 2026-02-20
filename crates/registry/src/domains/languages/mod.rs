//! Languages domain registration and runtime entry construction.

#[path = "compile/builtins.rs"]
pub mod builtins;
#[path = "compile/link.rs"]
pub mod link;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "runtime/view.rs"]
pub mod view;
#[path = "runtime/query.rs"]
pub mod query;
#[path = "contract/spec.rs"]
pub mod spec;
#[path = "contract/types.rs"]
pub mod types;
mod domain;

pub use domain::Languages;
pub use query::LanguagesRegistry;
pub use types::{LanguageEntry, LanguageInput};

/// Registers compiled languages from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_languages_spec();
	let linked = link::link_languages(&spec);

	for def in linked {
		db.push_domain::<Languages>(LanguageInput::Linked(def));
	}
}
