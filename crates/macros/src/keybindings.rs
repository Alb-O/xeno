//! KDL keybinding parsing macro.
//!
//! Parses keybindings in KDL format and emits a keybinding list per action.
//! Supports key sequences like `"g g"` for multi-key bindings.
//!
//! Note: Currently unused — all actions were migrated from the `action!` macro
//! to `action_handler!` + KDL metadata. Retained for potential future use.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Token, parse_macro_input};

/// Parses keybinding definitions and generates keybinding lists.
pub fn parse_keybindings(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ParseKeybindingsInput);

	match generate_keybindings(&input.action_name, &input.kdl_str, &input.action_id) {
		Ok(tokens) => tokens.into(),
		Err(e) => syn::Error::new(input.kdl_span, e).to_compile_error().into(),
	}
}

/// Parsed input for the keybindings macro.
struct ParseKeybindingsInput {
	/// Name of the action to bind.
	action_name: String,
	/// Raw KDL string containing binding definitions.
	kdl_str: String,
	/// Expression providing the canonical action ID string.
	action_id: Expr,
	/// Span for error reporting.
	kdl_span: proc_macro2::Span,
}

impl Parse for ParseKeybindingsInput {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let action_ident: syn::Ident = input.parse()?;
		input.parse::<Token![,]>()?;
		let kdl_lit: syn::LitStr = input.parse()?;
		input.parse::<Token![,]>()?;
		let action_id: syn::Expr = input.parse()?;

		Ok(ParseKeybindingsInput {
			action_name: action_ident.to_string(),
			kdl_str: kdl_lit.value(),
			action_id,
			kdl_span: kdl_lit.span(),
		})
	}
}

/// Generates a keybinding list for an action using `LazyLock`.
fn generate_keybindings(action_name: &str, kdl_str: &str, action_id: &Expr) -> Result<proc_macro2::TokenStream, String> {
	let doc: kdl::KdlDocument = kdl_str.parse().map_err(|e: kdl::KdlError| format!("KDL parse error: {e}"))?;

	let mut bindings = Vec::new();

	for node in doc.nodes() {
		let mode_name = node.name().value();

		let mode_variant = match mode_name {
			"normal" => quote! { Normal },
			"insert" => quote! { Insert },
			"window" => quote! { Window },
			"match" => quote! { Match },
			"space" => quote! { Space },
			other => {
				return Err(format!("Unknown mode: {other}. Valid modes: normal, insert, window, match, space"));
			}
		};

		for entry in node.entries().iter() {
			if entry.name().is_some() {
				continue;
			}

			let Some(key_str) = entry.value().as_string() else {
				continue;
			};

			// Validate the key sequence at compile time
			xeno_keymap_parser::parse_seq(key_str).map_err(|e| format!("Invalid key sequence \"{key_str}\": {e}"))?;

			bindings.push(quote! {
				xeno_registry::actions::KeyBindingDef {
					mode: xeno_registry::actions::BindingMode::#mode_variant,
					keys: std::sync::Arc::from(#key_str),
					action: std::sync::Arc::from(#action_id),
					priority: 100,
				}
			});
		}
	}

	let static_ident = format_ident!("KEYBINDINGS_{}", action_name);

	Ok(quote! {
		#[allow(non_upper_case_globals)]
		pub static #static_ident: std::sync::LazyLock<Vec<xeno_registry::actions::KeyBindingDef>> =
			std::sync::LazyLock::new(|| vec![
				#(#bindings),*
			]);
	})
}
