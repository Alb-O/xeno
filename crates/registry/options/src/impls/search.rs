//! Search-related options.

use crate::option;

option!(
	search_case_sensitive,
	Bool,
	false,
	Global,
	"Case-sensitive search by default"
);
option!(
	search_smart_case,
	Bool,
	true,
	Global,
	"Smart case: case-sensitive if pattern has uppercase"
);
option!(
	search_wrap,
	Bool,
	true,
	Global,
	"Wrap search around document boundaries"
);
option!(
	incremental_search,
	Bool,
	true,
	Global,
	"Show matches while typing search pattern"
);
