//! File handling options.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "backup", scope = global)]
/// Create backup files before saving.
pub static BACKUP: bool = false;

#[derive_option]
#[option(kdl = "undo-file", scope = global)]
/// Persist undo history to disk.
pub static UNDO_FILE: bool = false;

#[derive_option]
#[option(kdl = "auto-save", scope = global)]
/// Automatically save files on focus loss.
pub static AUTO_SAVE: bool = false;

#[derive_option]
#[option(kdl = "final-newline", scope = buffer)]
/// Ensure files end with a newline when saving.
pub static FINAL_NEWLINE: bool = true;

#[derive_option]
#[option(kdl = "trim-trailing-whitespace", scope = buffer)]
/// Remove trailing whitespace when saving.
pub static TRIM_TRAILING_WHITESPACE: bool = false;
