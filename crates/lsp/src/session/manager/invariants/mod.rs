//! Machine-checkable invariant catalog and proof entrypoints for LSP manager behavior.
#![allow(dead_code)]

pub(crate) mod catalog;

#[allow(unused_imports)]
pub(crate) use catalog::{
	ATOMIC_REGISTRY_MUTATION_ACROSS_INDICES, CLIENT_CAPABILITIES_ACCESS_IS_FALLIBLE,
	DOCUMENT_SYNC_DOES_NOT_NOTIFY_BEFORE_INIT, LANGUAGE_SERVER_ID_IS_SLOT_PLUS_GENERATION,
	LSP_SYSTEM_LOOKUPS_USE_CANONICAL_PATHS, POSITION_REQUESTS_GATE_ON_CLIENT_READINESS,
	READY_FLAG_REQUIRES_CAPABILITIES_WITH_RELEASE_ACQUIRE_ORDERING,
	ROUTER_DROPS_STALE_GENERATION_EVENTS, ROUTER_PROCESSES_EVENTS_SEQUENTIALLY_AND_REPLIES_INLINE,
	SERVER_CONFIG_CARRIES_PREASSIGNED_SERVER_ID, SINGLEFLIGHT_START_PER_LANGUAGE_AND_ROOT,
	STOPPED_OR_CRASHED_SERVER_IS_REMOVED_AND_PROGRESS_CLEARED,
	WORKSPACE_CONFIGURATION_RESPONSE_MATCHES_ITEM_COUNT,
	WORKSPACE_FOLDERS_RESPONSE_USES_PERCENT_ENCODED_URIS,
};

#[cfg(doc)]
pub(crate) fn test_registry_singleflight_prevents_duplicate_transport_start() {}

#[cfg(doc)]
pub(crate) fn test_registry_remove_server_scrubs_all_indices() {}

#[cfg(doc)]
pub(crate) async fn test_router_event_ordering() {}

#[cfg(doc)]
pub(crate) fn test_status_stopped_removes_server_and_clears_progress() {}

#[cfg(doc)]
pub(crate) fn test_router_drops_stale_generation_events() {}

#[cfg(doc)]
pub(crate) fn test_server_id_generation_increments_on_restart() {}

#[cfg(doc)]
pub(crate) fn test_singleflight_start() {}

#[cfg(doc)]
pub(crate) fn test_server_request_workspace_configuration_section_slicing() {}

#[cfg(doc)]
pub(crate) fn test_server_request_workspace_folders_uri_encoding() {}

#[cfg(doc)]
pub(crate) fn test_document_sync_returns_not_ready_before_init() {}

#[cfg(doc)]
pub(crate) fn test_prepare_position_request_returns_none_before_ready() {}

#[cfg(doc)]
pub(crate) fn test_client_handle_capabilities_returns_none_before_init() {}

#[cfg(doc)]
pub(crate) fn test_set_ready_requires_initialized() {}

#[cfg(doc)]
pub(crate) fn test_registry_lookup_uses_canonical_path() {}

#[cfg(test)]
mod proofs;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use proofs::{
	test_client_handle_capabilities_returns_none_before_init,
	test_document_sync_returns_not_ready_before_init,
	test_prepare_position_request_returns_none_before_ready,
	test_registry_lookup_uses_canonical_path, test_registry_remove_server_scrubs_all_indices,
	test_registry_singleflight_prevents_duplicate_transport_start,
	test_router_drops_stale_generation_events, test_router_event_ordering,
	test_server_id_generation_increments_on_restart,
	test_server_request_workspace_configuration_section_slicing,
	test_server_request_workspace_folders_uri_encoding, test_set_ready_requires_initialized,
	test_singleflight_start, test_status_stopped_removes_server_and_clears_progress,
};
