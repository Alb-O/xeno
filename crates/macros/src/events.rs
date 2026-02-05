//! Proc macro for generating hook event types and extractors.
//!
//! The `define_events!` macro generates from a single event definition:
//! - `HookEvent` enum
//! - `HookEventData<'a>` enum with borrowed payloads
//! - `OwnedHookContext` enum with owned payloads
//! - `__hook_extract!` macro for sync parameter extraction
//! - `__async_hook_extract!` macro for async parameter extraction

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Ident, LitStr, Result, Token, braced};

/// A single event definition parsed from the macro input.
struct EventDef {
	/// Outer attributes (e.g., doc comments) for the event.
	attrs: Vec<syn::Attribute>,
	/// The event variant name (e.g., `BufferOpen`).
	name: Ident,
	/// The event string identifier (e.g., `"buffer:open"`).
	event_str: LitStr,
	/// Fields in the event payload.
	fields: Vec<EventField>,
}

/// A field in an event payload.
struct EventField {
	/// Outer attributes (e.g., doc comments) for the field.
	attrs: Vec<syn::Attribute>,
	/// Field name (e.g., `path`).
	name: Ident,
	/// Type token that maps to borrowed/owned types (e.g., `Path`, `RopeSlice`).
	ty: Ident,
}

/// All event definitions from the macro input.
struct EventDefs {
	/// List of parsed event definitions.
	events: Vec<EventDef>,
}

impl Parse for EventField {
	fn parse(input: ParseStream) -> Result<Self> {
		let attrs = input.call(syn::Attribute::parse_outer)?;
		let name: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let ty: Ident = input.parse()?;
		Ok(EventField { attrs, name, ty })
	}
}

impl Parse for EventDef {
	fn parse(input: ParseStream) -> Result<Self> {
		let attrs = input.call(syn::Attribute::parse_outer)?;
		let name: Ident = input.parse()?;
		input.parse::<Token![=>]>()?;
		let event_str: LitStr = input.parse()?;

		let fields = if input.peek(syn::token::Brace) {
			let content;
			braced!(content in input);
			let fields: Punctuated<EventField, Token![,]> =
				content.parse_terminated(EventField::parse, Token![,])?;
			fields.into_iter().collect()
		} else {
			Vec::new()
		};

		Ok(EventDef {
			attrs,
			name,
			event_str,
			fields,
		})
	}
}

impl Parse for EventDefs {
	fn parse(input: ParseStream) -> Result<Self> {
		let events: Punctuated<EventDef, Token![,]> =
			input.parse_terminated(EventDef::parse, Token![,])?;
		Ok(EventDefs {
			events: events.into_iter().collect(),
		})
	}
}

/// Maps a field type token to its borrowed form.
fn borrowed_type(ty: &Ident) -> TokenStream2 {
	let ty_str = ty.to_string();
	match ty_str.as_str() {
		"Path" => quote! { &'a ::std::path::Path },
		"RopeSlice" => quote! { ::xeno_primitives::RopeSlice<'a> },
		"OptionStr" => quote! { ::core::option::Option<&'a str> },
		_ => quote! { #ty },
	}
}

/// Maps a field type token to its owned form.
fn owned_type(ty: &Ident) -> TokenStream2 {
	let ty_str = ty.to_string();
	match ty_str.as_str() {
		"Path" => quote! { ::std::path::PathBuf },
		"RopeSlice" => quote! { ::std::string::String },
		"OptionStr" => quote! { ::core::option::Option<::std::string::String> },
		_ => quote! { #ty },
	}
}

/// Generates the conversion expression for borrowed -> owned.
fn owned_value(ty: &Ident, field: &Ident) -> TokenStream2 {
	let ty_str = ty.to_string();
	match ty_str.as_str() {
		"Path" => quote! { #field.to_path_buf() },
		"RopeSlice" => quote! { #field.to_string() },
		"OptionStr" => quote! { #field.map(::std::string::String::from) },
		_ => quote! { #field.clone() },
	}
}

/// Entry point for the `define_events!` proc macro.
///
/// Generates `HookEvent`, `HookEventData`, `OwnedHookContext` enums and
/// extraction macros from event definitions.
pub fn define_events(input: TokenStream) -> TokenStream {
	let EventDefs { events } = syn::parse_macro_input!(input as EventDefs);

	// Generate HookEvent enum variants
	let event_variants: Vec<_> = events
		.iter()
		.map(|e| {
			let name = &e.name;
			let attrs = &e.attrs;
			quote! {
				#(#attrs)*
				#name
			}
		})
		.collect();

	// Generate HookEvent::as_str match arms
	let event_str_arms: Vec<_> = events
		.iter()
		.map(|e| {
			let name = &e.name;
			let s = &e.event_str;
			quote! { HookEvent::#name => #s }
		})
		.collect();

	// Generate HookEventData variants
	let event_data_variants: Vec<_> = events
		.iter()
		.map(|e| {
			let name = &e.name;
			let attrs = &e.attrs;
			if e.fields.is_empty() {
				quote! {
					#(#attrs)*
					#name
				}
			} else {
				let fields: Vec<_> = e
					.fields
					.iter()
					.map(|f| {
						let fattrs = &f.attrs;
						let fname = &f.name;
						let fty = borrowed_type(&f.ty);
						quote! { #(#fattrs)* #fname: #fty }
					})
					.collect();
				quote! {
					#(#attrs)*
					#name { #(#fields),* }
				}
			}
		})
		.collect();

	// Generate HookEventData::event() match arms
	let event_data_event_arms: Vec<_> = events
		.iter()
		.map(|e| {
			let name = &e.name;
			if e.fields.is_empty() {
				quote! { HookEventData::#name => HookEvent::#name }
			} else {
				quote! { HookEventData::#name { .. } => HookEvent::#name }
			}
		})
		.collect();

	// Generate From<&HookEventData> for OwnedHookContext match arms
	let from_arms: Vec<_> = events
		.iter()
		.map(|e| {
			let name = &e.name;
			if e.fields.is_empty() {
				quote! {
					HookEventData::#name => OwnedHookContext::#name
				}
			} else {
				let field_names: Vec<_> = e.fields.iter().map(|f| &f.name).collect();
				let field_conversions: Vec<_> = e
					.fields
					.iter()
					.map(|f| {
						let fname = &f.name;
						let conv = owned_value(&f.ty, fname);
						quote! { #fname: #conv }
					})
					.collect();
				quote! {
					HookEventData::#name { #(#field_names),* } => {
						OwnedHookContext::#name { #(#field_conversions),* }
					}
				}
			}
		})
		.collect();

	// Generate OwnedHookContext variants
	let owned_variants: Vec<_> = events
		.iter()
		.map(|e| {
			let name = &e.name;
			let attrs = &e.attrs;
			if e.fields.is_empty() {
				quote! {
					#(#attrs)*
					#name
				}
			} else {
				let fields: Vec<_> = e
					.fields
					.iter()
					.map(|f| {
						let fattrs = &f.attrs;
						let fname = &f.name;
						let fty = owned_type(&f.ty);
						quote! { #(#fattrs)* #fname: #fty }
					})
					.collect();
				quote! {
					#(#attrs)*
					#name { #(#fields),* }
				}
			}
		})
		.collect();

	// Generate OwnedHookContext::event() match arms
	let owned_event_arms: Vec<_> = events
		.iter()
		.map(|e| {
			let name = &e.name;
			if e.fields.is_empty() {
				quote! { OwnedHookContext::#name => HookEvent::#name }
			} else {
				quote! { OwnedHookContext::#name { .. } => HookEvent::#name }
			}
		})
		.collect();

	// Generate __hook_extract! macro arms
	// Use $crate:: which resolves to the invoking crate
	let hook_extract_arms: Vec<_> = events
		.iter()
		.map(|e| {
			let name = &e.name;
			if e.fields.is_empty() {
				quote! {
					(#name, $ctx:ident $(,)?) => {
						let $crate::HookEventData::#name = &$ctx.data else {
							return $crate::HookAction::Done($crate::HookResult::Continue);
						};
					};
				}
			} else {
				quote! {
					(#name, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
						let $crate::HookEventData::#name { $($param,)* .. } = &$ctx.data else {
							return $crate::HookAction::Done($crate::HookResult::Continue);
						};
						$(let $param: $ty = $param; )*
					};
				}
			}
		})
		.collect();

	// Generate __async_hook_extract! macro arms
	let async_hook_extract_arms: Vec<_> = events
		.iter()
		.map(|e| {
			let name = &e.name;
			if e.fields.is_empty() {
				quote! {
					(#name, $owned:ident $(,)?) => {
						let $crate::OwnedHookContext::#name = $owned else {
							return $crate::HookResult::Continue;
						};
					};
				}
			} else {
				quote! {
					(#name, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
						let $crate::OwnedHookContext::#name { $($param,)* .. } = $owned else {
							return $crate::HookResult::Continue;
						};
						$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
					};
				}
			}
		})
		.collect();

	let output = quote! {
		/// Discriminant for hook event types.
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
		pub enum HookEvent {
			#(#event_variants),*
		}

		impl HookEvent {
			/// Returns the string identifier for this event type.
			pub fn as_str(&self) -> &'static str {
				match self {
					#(#event_str_arms),*
				}
			}
		}

		/// Event-specific data for hooks.
		///
		/// Contains the payload for each hook event type.
		pub enum HookEventData<'a> {
			#(#event_data_variants),*
		}

		impl<'a> HookEventData<'a> {
			/// Returns the event type for this data.
			pub fn event(&self) -> HookEvent {
				match self {
					#(#event_data_event_arms),*
				}
			}

			/// Creates an owned version of this event data for use in async hooks.
			pub fn to_owned(&self) -> OwnedHookContext {
				OwnedHookContext::from(self)
			}
		}

		impl<'a> From<&HookEventData<'a>> for OwnedHookContext {
			fn from(data: &HookEventData<'a>) -> Self {
				match data {
					#(#from_arms),*
				}
			}
		}

		/// Owned version of [`HookContext`] for async hook handlers.
		#[derive(Debug, Clone)]
		pub enum OwnedHookContext {
			#(#owned_variants),*
		}

		impl OwnedHookContext {
			/// Returns the event type for this context.
			pub fn event(&self) -> HookEvent {
				match self {
					#(#owned_event_arms),*
				}
			}
		}

		/// Extracts event parameters in sync hook handlers.
		#[doc(hidden)]
		#[macro_export]
		macro_rules! __hook_extract {
			#(#hook_extract_arms)*
		}

		/// Extracts event parameters in async hook handlers.
		#[doc(hidden)]
		#[macro_export]
		macro_rules! __async_hook_extract {
			#(#async_hook_extract_arms)*
		}
	};

	output.into()
}
