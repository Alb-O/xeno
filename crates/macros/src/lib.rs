//! Procedural macros for Xeno editor.
//!
//! Provides derive macros and attribute macros:
//! * `#[derive(DispatchResult)]` - generates result handler registries
//! * `#[derive(Option)]` - option registration from static definitions
//! * `define_events!` - hook event generation

use proc_macro::TokenStream;

mod dispatch;
mod events;
/// Notification macro implementation.
mod notification;
/// Option derive macro implementation.
mod option;

/// Registers a notification type with the notification system.
///
/// ```ignore
/// register_notification!(INFO_NOTIFICATION, "info",
///     level: xeno_registry::notifications::Level::Info,
///     semantic: xeno_registry::themes::SEMANTIC_INFO
/// );
/// ```
#[proc_macro]
pub fn register_notification(input: TokenStream) -> TokenStream {
	notification::register_notification(input)
}

/// Derives dispatch infrastructure for `ActionResult`.
///
/// Generates handler registry declarations (`RESULT_*_HANDLERS`) and a `dispatch_result`
/// function.
///
/// Attributes:
/// * `#[handler(Foo)]` - Use `RESULT_FOO_HANDLERS` instead of deriving from variant name
///
/// ```ignore
/// #[derive(DispatchResult)]
/// pub enum ActionResult {
///     Ok,
///     #[handler(Quit)]
///     Quit,
///     Motion(Selection),
/// }
/// ```
#[proc_macro_derive(DispatchResult, attributes(handler, handler_coverage))]
pub fn derive_dispatch_result(input: TokenStream) -> TokenStream {
	dispatch::derive_dispatch_result(input)
}

/// Generates hook event types and extractor macros from a single definition.
///
/// This proc macro is the single source of truth for hook events. It generates:
/// * `HookEvent` enum for event discrimination
/// * `HookEventData<'a>` enum with borrowed event payloads
/// * `OwnedHookContext` enum with owned payloads for async
/// * `__hook_extract!` macro for sync parameter extraction
/// * `__async_hook_extract!` macro for async parameter extraction
///
/// # Example
///
/// ```ignore
/// define_events! {
///     /// Editor is starting up.
///     EditorStart => "editor:start",
///     
///     /// A buffer was opened.
///     BufferOpen => "buffer:open" {
///         path: Path,
///         text: RopeSlice,
///         file_type: OptionStr,
///     },
/// }
/// ```
///
/// # Field Types
///
/// Special type tokens are mapped to borrowed/owned forms:
/// * `Path` → `&Path` / `PathBuf`
/// * `RopeSlice` → `RopeSlice<'a>` / `String`
/// * `OptionStr` → `Option<&str>` / `Option<String>`
/// * Other types are used as-is (must implement `Clone`)
#[proc_macro]
pub fn define_events(input: TokenStream) -> TokenStream {
	events::define_events(input)
}

/// Registers a configuration option from a static definition.
///
/// Transforms a static item into an option registration:
///
/// ```ignore
/// #[derive(Option)]
/// #[option(key = "tab-width", scope = buffer)]
/// /// Number of spaces a tab character occupies.
/// pub static TAB_WIDTH: i64 = 4;
/// ```
///
/// Generates:
/// * Static `OptionDef` registered in the `OPTIONS` slice
/// * Typed `TypedOptionKey<T>` constant for compile-time type safety
///
/// # Attributes
///
/// - `key = "key-name"` - Required: configuration key
/// - `scope = global | buffer` - Required: Option scope
/// - `priority = N` - Optional: Sort priority (default 0)
///
/// # Supported Types
///
/// * `i64` → `OptionType::Int`
/// * `bool` → `OptionType::Bool`
/// * `String` → `OptionType::String`
/// * `&'static str` → `OptionType::String` (converted to owned)
#[proc_macro_attribute]
pub fn derive_option(_attr: TokenStream, item: TokenStream) -> TokenStream {
	option::derive_option(item)
}
