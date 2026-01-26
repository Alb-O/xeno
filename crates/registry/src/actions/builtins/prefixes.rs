//! Key sequence prefix definitions for the which-key HUD.

use crate::actions::key_prefix;

key_prefix!(normal "g" as g => "Goto");
key_prefix!(normal "z" as z => "View");
key_prefix!(normal "ctrl-w" as ctrl_w => "Window");
key_prefix!(normal "ctrl-w s" as ctrl_w_s => "Split");
key_prefix!(normal "ctrl-w f" as ctrl_w_f => "Focus");
key_prefix!(normal "ctrl-w c" as ctrl_w_c => "Close");
