use crate::actions::key_prefix;
use crate::actions::keybindings::KeyPrefixDef;
use crate::db::builder::RegistryDbBuilder;

key_prefix!(normal "ctrl-w" as ctrl_w => "Window");
key_prefix!(normal "ctrl-w f" as ctrl_w_f => "Focus");
key_prefix!(normal "ctrl-w s" as ctrl_w_s => "Split");
key_prefix!(normal "ctrl-w c" as ctrl_w_c => "Close");
key_prefix!(normal "g" => "Goto");
key_prefix!(normal "z" => "View");

pub(super) const PREFIXES: &[KeyPrefixDef] = &[
	KEY_PREFIX_NORMAL_CTRL_W,
	KEY_PREFIX_NORMAL_CTRL_W_F,
	KEY_PREFIX_NORMAL_CTRL_W_S,
	KEY_PREFIX_NORMAL_CTRL_W_C,
	KEY_PREFIX_NORMAL_g,
	KEY_PREFIX_NORMAL_z,
];

pub(super) fn register_prefixes(builder: &mut RegistryDbBuilder) {
	builder.register_key_prefixes(PREFIXES);
}
