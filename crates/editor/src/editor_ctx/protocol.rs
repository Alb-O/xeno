use xeno_registry::actions::ActionEffects;

/// Effects command envelope consumed by the editor-context interpreter boundary.
#[derive(Debug, Clone)]
pub enum EffectsCmd {
	Apply(ActionEffectsEnvelope),
}

/// Action effects payload carried by [`EffectsCmd::Apply`].
#[derive(Debug, Clone)]
pub struct ActionEffectsEnvelope {
	pub effects: ActionEffects,
	pub extend: bool,
}

/// Effects event envelope emitted by the editor-context interpreter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsEvt {
	Applied { should_quit: bool },
}

impl EffectsEvt {
	pub const fn should_quit(self) -> bool {
		match self {
			Self::Applied { should_quit } => should_quit,
		}
	}
}
