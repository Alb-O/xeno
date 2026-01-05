//! Search-related options.

use xeno_macro::derive_option;

#[derive_option]
#[option(kdl = "search-case-sensitive", scope = global)]
/// Case-sensitive search by default.
pub static SEARCH_CASE_SENSITIVE: bool = false;

#[derive_option]
#[option(kdl = "search-smart-case", scope = global)]
/// Smart case: case-sensitive if pattern has uppercase.
pub static SEARCH_SMART_CASE: bool = true;

#[derive_option]
#[option(kdl = "search-wrap", scope = global)]
/// Wrap search around document boundaries.
pub static SEARCH_WRAP: bool = true;

#[derive_option]
#[option(kdl = "incremental-search", scope = global)]
/// Show matches while typing search pattern.
pub static INCREMENTAL_SEARCH: bool = true;
