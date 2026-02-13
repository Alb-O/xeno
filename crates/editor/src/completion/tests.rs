use super::{frizbee_config_for_query, frizbee_match, max_typos_for_query};

#[test]
fn matching_allows_single_typo_by_default() {
	assert!(frizbee_match("rtgistry", "registry_diag").is_some());
}

#[test]
fn short_queries_do_not_allow_typos() {
	assert!(frizbee_match("ab", "a").is_none());
}

#[test]
fn typo_budget_scales_with_query_length() {
	assert_eq!(max_typos_for_query("abc"), 0);
	assert_eq!(max_typos_for_query("abcd"), 1);
	assert_eq!(max_typos_for_query("abcdefghij"), 2);

	assert_eq!(frizbee_config_for_query("ab").max_typos, Some(0));
	assert_eq!(frizbee_config_for_query("rtgistry").max_typos, Some(1));
	assert_eq!(frizbee_config_for_query("very_long_query").max_typos, Some(2));
}
