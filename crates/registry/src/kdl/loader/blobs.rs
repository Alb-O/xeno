pub(super) static ACTIONS_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/actions.bin"));
pub(super) static COMMANDS_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/commands.bin"));
pub(super) static MOTIONS_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/motions.bin"));
pub(super) static TEXT_OBJECTS_BLOB: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/text_objects.bin"));
pub(super) static OPTIONS_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/options.bin"));
pub(super) static GUTTERS_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/gutters.bin"));
pub(super) static STATUSLINE_BLOB: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/statusline.bin"));
pub(super) static HOOKS_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/hooks.bin"));
pub(super) static NOTIFICATIONS_BLOB: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/notifications.bin"));
pub(super) static THEMES_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/themes.bin"));
