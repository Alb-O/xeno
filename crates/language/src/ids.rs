use tree_house::Language as ThLanguage;
use xeno_registry::{DenseId, LanguageId};

/// Conversions are intentionally explicit and checked.
pub trait RegistryLanguageIdExt {
	fn to_tree_house(self) -> ThLanguage;
}

impl RegistryLanguageIdExt for LanguageId {
	fn to_tree_house(self) -> ThLanguage {
		ThLanguage::new(self.as_u32())
	}
}

pub trait TreeHouseLanguageExt {
	fn to_registry(self, max: u32) -> Option<LanguageId>;
}

impl TreeHouseLanguageExt for ThLanguage {
	fn to_registry(self, max: u32) -> Option<LanguageId> {
		let idx = self.idx() as u32;
		(idx < max).then(|| LanguageId::from_u32(idx))
	}
}
