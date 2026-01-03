//! KDL keybinding parsing macro.
//!
//! Parses keybindings in KDL format and emits them to the KEYBINDINGS distributed slice.
//! Supports key sequences like `"g g"` for multi-key bindings.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Token, parse_macro_input};

use crate::dispatch::to_screaming_snake_case;

/// Parses keybinding definitions and generates distributed slice entries.
pub fn parse_keybindings(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ParseKeybindingsInput);

	match generate_keybindings(&input.action_name, &input.kdl_str) {
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
	/// Span for error reporting.
	kdl_span: proc_macro2::Span,
}

impl Parse for ParseKeybindingsInput {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let action_ident: syn::Ident = input.parse()?;
		input.parse::<Token![,]>()?;
		let kdl_lit: syn::LitStr = input.parse()?;

		Ok(ParseKeybindingsInput {
			action_name: action_ident.to_string(),
			kdl_str: kdl_lit.value(),
			kdl_span: kdl_lit.span(),
		})
	}
}

/// Generates static keybinding entries for the distributed slice.
fn generate_keybindings(
	action_name: &str,
	kdl_str: &str,
) -> Result<proc_macro2::TokenStream, String> {
	let doc: kdl::KdlDocument = kdl_str
		.parse()
		.map_err(|e: kdl::KdlError| format!("KDL parse error: {e}"))?;

	let mut statics = Vec::new();
	let action_upper = to_screaming_snake_case(action_name);

	for node in doc.nodes() {
		let mode_name = node.name().value();
		let mode_upper = mode_name.to_uppercase();

		let mode_variant = match mode_name {
			"normal" => quote! { Normal },
			"insert" => quote! { Insert },
			"window" => quote! { Window },
			"match" => quote! { Match },
			"space" => quote! { Space },
			other => {
				return Err(format!(
					"Unknown mode: {other}. Valid modes: normal, insert, window, match, space"
				));
			}
		};

		for (idx, entry) in node.entries().iter().enumerate() {
			if entry.name().is_some() {
				continue;
			}

			let Some(key_str) = entry.value().as_string() else {
				continue;
			};

			// Validate the key sequence at compile time
			evildoer_keymap_parser::parse_seq(key_str)
				.map_err(|e| format!("Invalid key sequence \"{key_str}\": {e}"))?;

			let static_ident = format_ident!("KB_{}_{}__{}", action_upper, mode_upper, idx);

			statics.push(quote! {
				#[allow(non_upper_case_globals)]
				#[::linkme::distributed_slice(evildoer_registry_actions::keybindings::KEYBINDINGS)]
				static #static_ident: evildoer_registry_actions::keybindings::KeyBindingDef =
					evildoer_registry_actions::keybindings::KeyBindingDef {
						mode: evildoer_registry_actions::keybindings::BindingMode::#mode_variant,
						keys: #key_str,
						action: #action_name,
						priority: 100,
					};
			});
		}
	}

	Ok(quote! { #(#statics)* })
}
