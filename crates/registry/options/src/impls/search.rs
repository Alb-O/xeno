//! Search-related options.

use crate::option;

option!(search_case_sensitive, {
	kdl: "search-case-sensitive",
	type: Bool,
	default: false,
	scope: Global,
	description: "Case-sensitive search by default",
});

option!(search_smart_case, {
	kdl: "search-smart-case",
	type: Bool,
	default: true,
	scope: Global,
	description: "Smart case: case-sensitive if pattern has uppercase",
});

option!(search_wrap, {
	kdl: "search-wrap",
	type: Bool,
	default: true,
	scope: Global,
	description: "Wrap search around document boundaries",
});

option!(incremental_search, {
	kdl: "incremental-search",
	type: Bool,
	default: true,
	scope: Global,
	description: "Show matches while typing search pattern",
});
