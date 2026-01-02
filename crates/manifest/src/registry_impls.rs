//! [`RegistryMetadata`] implementations for registry definition types.

use evildoer_registry::actions::ActionDef;
use evildoer_registry::commands::CommandDef;
use evildoer_registry::hooks::HookDef;
use evildoer_registry::motions::MotionDef;
use evildoer_registry::notifications::NotificationTypeDef;
use evildoer_registry::options::OptionDef;
use evildoer_registry::panels::PanelDef;
use evildoer_registry::statusline::StatuslineSegmentDef;
use evildoer_registry::text_objects::TextObjectDef;

macro_rules! impl_registry_metadata {
	($type:ty) => {
		impl crate::RegistryMetadata for $type {
			fn id(&self) -> &'static str {
				self.id
			}
			fn name(&self) -> &'static str {
				self.name
			}
			fn priority(&self) -> i16 {
				self.priority
			}
			fn source(&self) -> crate::RegistrySource {
				self.source
			}
		}
	};
}

impl_registry_metadata!(ActionDef);
impl_registry_metadata!(CommandDef);
impl_registry_metadata!(HookDef);
impl_registry_metadata!(MotionDef);
impl_registry_metadata!(NotificationTypeDef);
impl_registry_metadata!(OptionDef);
impl_registry_metadata!(PanelDef);
impl_registry_metadata!(StatuslineSegmentDef);
impl_registry_metadata!(TextObjectDef);
