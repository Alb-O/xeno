//! Procedural macros for Xeno editor.
//!
//! Provides derive macros and attribute macros:
//! - `#[derive(DispatchResult)]` - generates result handler slices
//! - `#[derive(Option)]` - option registration from static definitions
//! - `#[extension]` - extension registration
//! - `define_events!` - hook event generation

use proc_macro::TokenStream;

mod dispatch;
mod events;
/// Extension attribute macro implementation.
mod extension;
mod keybindings;
/// Notification macro implementation.
mod notification;
/// Option derive macro implementation.
mod option;

/// Generates extension registrations from an `impl` block.
///
/// Supports `#[init]`, `#[render]`, `#[command]`, and `#[hook]` method attributes.
#[proc_macro_attribute]
pub fn extension(attr: TokenStream, item: TokenStream) -> TokenStream {
	extension::extension(attr, item)
}

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
/// Generates handler slice declarations (`RESULT_*_HANDLERS`), a `dispatch_result`
/// function, and `is_terminal_safe` method.
///
/// Attributes:
/// - `#[handler(Foo)]` - Use `RESULT_FOO_HANDLERS` instead of deriving from variant name
/// - `#[terminal_safe]` - Mark variant as safe to execute when terminal is focused
///
/// ```ignore
/// #[derive(DispatchResult)]
/// pub enum ActionResult {
///     #[terminal_safe]
///     Ok,
///     #[handler(Quit)]
///     Quit,
///     Motion(Selection),
/// }
/// ```
#[proc_macro_derive(DispatchResult, attributes(handler, terminal_safe, handler_coverage))]
pub fn derive_dispatch_result(input: TokenStream) -> TokenStream {
	dispatch::derive_dispatch_result(input)
}

/// Parses KDL keybinding definitions at compile time.
///
/// ```kdl
/// normal "h" "left" "ctrl-h"
/// insert "left"
/// window "s"
/// ```
///
/// Called internally by `action!` macro:
///
/// ```ignore
/// action!(
///     move_left,
///     { description: "Move cursor left", bindings: r#"normal "h" "left""# },
///     |ctx| { ... }
/// );
/// ```
#[proc_macro]
pub fn parse_keybindings(input: TokenStream) -> TokenStream {
	keybindings::parse_keybindings(input)
}

/// Generates hook event types and extractor macros from a single definition.
///
/// This proc macro is the single source of truth for hook events. It generates:
/// - `HookEvent` enum for event discrimination
/// - `HookEventData<'a>` enum with borrowed event payloads
/// - `OwnedHookContext` enum with owned payloads for async
/// - `__hook_extract!` macro for sync parameter extraction
/// - `__async_hook_extract!` macro for async parameter extraction
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
/// - `Path` → `&Path` / `PathBuf`
/// - `RopeSlice` → `RopeSlice<'a>` / `String`
/// - `OptionStr` → `Option<&str>` / `Option<String>`
/// - Other types are used as-is (must implement `Clone`)
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
/// #[option(kdl = "tab-width", scope = buffer)]
/// /// Number of spaces a tab character occupies.
/// pub static TAB_WIDTH: i64 = 4;
/// ```
///
/// Generates:
/// - Static `OptionDef` registered in the `OPTIONS` slice
/// - Typed `TypedOptionKey<T>` constant for compile-time type safety
///
/// # Attributes
///
/// - `kdl = "key"` - Required: KDL configuration key
/// - `scope = global | buffer` - Required: Option scope
/// - `priority = N` - Optional: Sort priority (default 0)
///
/// # Supported Types
///
/// - `i64` → `OptionType::Int`
/// - `bool` → `OptionType::Bool`
/// - `String` → `OptionType::String`
/// - `&'static str` → `OptionType::String` (converted to owned)
#[proc_macro_attribute]
pub fn derive_option(_attr: TokenStream, item: TokenStream) -> TokenStream {
	option::derive_option(item)
}
