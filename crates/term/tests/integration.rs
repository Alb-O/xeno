#![allow(unused_crate_dependencies)]
//! Integration test suite for xeno-term.

#[path = "integration/completion.rs"]
mod completion;
#[path = "integration/helpers.rs"]
mod helpers;
#[path = "integration/kitty_ltier_stale_syntax.rs"]
mod kitty_ltier_stale_syntax;
#[path = "integration/kitty_multiselect.rs"]
mod kitty_multiselect;
#[path = "integration/kitty_separator_animation.rs"]
mod kitty_separator_animation;
#[path = "integration/kitty_split_junctions.rs"]
mod kitty_split_junctions;
#[path = "integration/kitty_split_resize.rs"]
mod kitty_split_resize;
#[path = "integration/kitty_viewport_stability.rs"]
mod kitty_viewport_stability;
