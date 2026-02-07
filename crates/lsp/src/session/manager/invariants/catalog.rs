//! Invariant catalog for [`crate::session::manager::LspManager`].
#![allow(dead_code)]

/// Must singleflight `transport.start()` per `(language, root_path)` key.
///
/// - Enforced in: [`crate::registry::Registry::get_or_start`]
/// - Tested by: [`crate::session::manager::invariants::test_registry_singleflight_prevents_duplicate_transport_start`]
/// - Failure symptom: Duplicate server starts and inconsistent IDs for identical startup requests.
pub(crate) const SINGLEFLIGHT_START_PER_LANGUAGE_AND_ROOT: () = ();

/// Must mutate `servers`, `server_meta`, and `id_index` atomically.
///
/// - Enforced in: [`crate::registry::Registry::get_or_start`], [`crate::registry::Registry::remove_server`]
/// - Tested by: [`crate::session::manager::invariants::test_registry_remove_server_scrubs_all_indices`]
/// - Failure symptom: Stale metadata or lookup mismatches after removals.
pub(crate) const ATOMIC_REGISTRY_MUTATION_ACROSS_INDICES: () = ();

/// Router must process transport events sequentially and reply to server requests inline.
///
/// - Enforced in: [`crate::session::manager::LspManager::spawn_router`]
/// - Tested by: [`crate::session::manager::invariants::test_router_event_ordering`]
/// - Failure symptom: Request/reply pairing breaks and servers hang waiting for responses.
pub(crate) const ROUTER_PROCESSES_EVENTS_SEQUENTIALLY_AND_REPLIES_INLINE: () = ();

/// Must remove stopped/crashed servers and clear per-server progress.
///
/// - Enforced in: [`crate::session::manager::LspManager::spawn_router`], [`crate::registry::Registry::remove_server`]
/// - Tested by: [`crate::session::manager::invariants::test_status_stopped_removes_server_and_clears_progress`]
/// - Failure symptom: Dead servers remain reachable and progress UI gets stuck.
pub(crate) const STOPPED_OR_CRASHED_SERVER_IS_REMOVED_AND_PROGRESS_CLEARED: () = ();

/// Router must discard events from stale server generations.
///
/// - Enforced in: [`crate::session::manager::LspManager::spawn_router`], [`crate::registry::Registry::is_current`]
/// - Tested by: [`crate::session::manager::invariants::test_router_drops_stale_generation_events`]
/// - Failure symptom: Diagnostics or progress from old processes leak into active sessions.
pub(crate) const ROUTER_DROPS_STALE_GENERATION_EVENTS: () = ();

/// [`crate::client::LanguageServerId`] must represent `(slot, generation)`.
///
/// - Enforced in: `RegistryState::get_or_create_slot_id`, `RegistryState::next_gen`
/// - Tested by: [`crate::session::manager::invariants::test_server_id_generation_increments_on_restart`]
/// - Failure symptom: Restarted servers alias old IDs and stale event filtering fails.
pub(crate) const LANGUAGE_SERVER_ID_IS_SLOT_PLUS_GENERATION: () = ();

/// `ServerConfig` must carry a pre-assigned [`crate::client::LanguageServerId`].
///
/// - Enforced in: [`crate::registry::Registry::get_or_start`], `LocalTransport` `LspTransport::start` impl
/// - Tested by: [`crate::session::manager::invariants::test_singleflight_start`]
/// - Failure symptom: Transport-generated IDs diverge from registry IDs.
pub(crate) const SERVER_CONFIG_CARRIES_PREASSIGNED_SERVER_ID: () = ();

/// `workspace/configuration` responses must return one entry per request item.
///
/// - Enforced in: `handle_workspace_configuration`
/// - Tested by: [`crate::session::manager::invariants::test_server_request_workspace_configuration_section_slicing`]
/// - Failure symptom: Servers reject config payloads and disable features.
pub(crate) const WORKSPACE_CONFIGURATION_RESPONSE_MATCHES_ITEM_COUNT: () = ();

/// `workspace/workspaceFolders` responses must use percent-encoded file URIs.
///
/// - Enforced in: `handle_workspace_folders`
/// - Tested by: [`crate::session::manager::invariants::test_server_request_workspace_folders_uri_encoding`]
/// - Failure symptom: Workspace roots with spaces or unicode are parsed incorrectly.
pub(crate) const WORKSPACE_FOLDERS_RESPONSE_USES_PERCENT_ENCODED_URIS: () = ();

/// [`crate::sync::DocumentSync`] must not notify changes before initialization completes.
///
/// - Enforced in: [`crate::sync::DocumentSync::notify_change_full_text`], [`crate::sync::DocumentSync::notify_change_incremental_no_content`]
/// - Tested by: [`crate::session::manager::invariants::test_document_sync_returns_not_ready_before_init`]
/// - Failure symptom: Changes are dropped or applied out of order.
pub(crate) const DOCUMENT_SYNC_DOES_NOT_NOTIFY_BEFORE_INIT: () = ();

/// Position-based requests must gate on [`crate::client::ClientHandle::is_ready`].
///
/// - Enforced in: `LspSystem::prepare_position_request` (in xeno-editor)
/// - Tested by: [`crate::session::manager::invariants::test_prepare_position_request_returns_none_before_ready`]
/// - Failure symptom: Requests are sent to uninitialized servers.
pub(crate) const POSITION_REQUESTS_GATE_ON_CLIENT_READINESS: () = ();

/// [`crate::client::ClientHandle::capabilities`] must be fallible and never panic.
///
/// - Enforced in: [`crate::client::ClientHandle::capabilities`], [`crate::client::ClientHandle::offset_encoding`]
/// - Tested by: [`crate::session::manager::invariants::test_client_handle_capabilities_returns_none_before_init`]
/// - Failure symptom: Capability reads panic before initialize completes.
pub(crate) const CLIENT_CAPABILITIES_ACCESS_IS_FALLIBLE: () = ();

/// Ready flag writes and reads must use Release/Acquire ordering and require initialized capabilities.
///
/// - Enforced in: [`crate::client::ClientHandle::set_ready`], [`crate::client::ClientHandle::is_ready`]
/// - Tested by: [`crate::session::manager::invariants::test_set_ready_requires_initialized`]
/// - Failure symptom: `is_ready == true` can race with missing capabilities.
pub(crate) const READY_FLAG_REQUIRES_CAPABILITIES_WITH_RELEASE_ACQUIRE_ORDERING: () = ();

/// LspSystem registry lookups must use canonicalized paths.
///
/// - Enforced in: `LspSystem::prepare_position_request`, `LspSystem::offset_encoding_for_buffer`, `LspSystem::incremental_encoding` (in xeno-editor)
/// - Tested by: [`crate::session::manager::invariants::test_registry_lookup_uses_canonical_path`]
/// - Failure symptom: Registry misses cause dropped requests or wrong encoding fallbacks.
pub(crate) const LSP_SYSTEM_LOOKUPS_USE_CANONICAL_PATHS: () = ();
