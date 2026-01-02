//! ActionResult dispatch derive macro.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Expr, Lit, Meta, parse_macro_input};

pub fn derive_dispatch_result(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let enum_name = &input.ident;

	let Data::Enum(data) = &input.data else {
		return syn::Error::new_spanned(&input, "DispatchResult can only be derived for enums")
			.to_compile_error()
			.into();
	};

	let mut slice_names: Vec<syn::Ident> = Vec::new();
	let mut match_arms = Vec::new();
	let mut terminal_safe_variants = Vec::new();
	let mut coverage_checks = Vec::new();

	let mut coverage_error = false;
	for attr in &input.attrs {
		if attr.path().is_ident("handler_coverage") {
			let Meta::NameValue(meta) = &attr.meta else {
				return syn::Error::new_spanned(
					attr,
					"handler_coverage must be a name-value attribute",
				)
				.to_compile_error()
				.into();
			};
			let Expr::Lit(expr) = &meta.value else {
				return syn::Error::new_spanned(attr, "handler_coverage must be a string")
					.to_compile_error()
					.into();
			};
			let Lit::Str(lit) = &expr.lit else {
				return syn::Error::new_spanned(attr, "handler_coverage must be a string")
					.to_compile_error()
					.into();
			};
			let value = lit.value();
			if value == "error" {
				coverage_error = true;
			} else {
				return syn::Error::new_spanned(attr, "handler_coverage must be \"error\"")
					.to_compile_error()
					.into();
			}
		}
	}

	for variant in &data.variants {
		let variant_name = &variant.ident;

		let mut handler_name = None;
		let mut coverage_ignore = false;
		for attr in &variant.attrs {
			if attr.path().is_ident("handler")
				&& let Ok(ident) = attr.parse_args::<syn::Ident>()
			{
				if ident == "ignore" {
					coverage_ignore = true;
				} else {
					handler_name = Some(ident);
				}
			}
		}
		let handler_name = handler_name.unwrap_or_else(|| variant_name.clone());

		let is_terminal_safe = variant
			.attrs
			.iter()
			.any(|attr| attr.path().is_ident("terminal_safe"));

		if is_terminal_safe {
			let pattern = match &variant.fields {
				syn::Fields::Unit => quote! { Self::#variant_name },
				syn::Fields::Unnamed(_) => quote! { Self::#variant_name(..) },
				syn::Fields::Named(_) => quote! { Self::#variant_name { .. } },
			};
			terminal_safe_variants.push(pattern);
		}

		let handler_screaming = to_screaming_snake_case(&handler_name.to_string());
		let slice_ident = format_ident!("RESULT_{}_HANDLERS", handler_screaming);

		if !slice_names.contains(&slice_ident) {
			slice_names.push(slice_ident.clone());
		}

		if coverage_error && !coverage_ignore {
			coverage_checks.push(quote! {
				if #slice_ident.is_empty() {
					missing.push(stringify!(#variant_name));
				}
			});
		}

		let pattern = match &variant.fields {
			syn::Fields::Unit => quote! { #enum_name::#variant_name },
			syn::Fields::Unnamed(_) => quote! { #enum_name::#variant_name(..) },
			syn::Fields::Named(_) => quote! { #enum_name::#variant_name { .. } },
		};

		match_arms.push(quote! {
			#pattern => run_handlers(&#slice_ident, result, ctx, extend)
		});
	}

	let coverage_test = if coverage_error {
		let coverage_fn = format_ident!(
			"handler_coverage_{}",
			to_screaming_snake_case(&enum_name.to_string()).to_lowercase()
		);
		quote! {
			#[cfg(test)]
			#[test]
			fn #coverage_fn() {
				let mut total = 0usize;
				#(total += #slice_names.len();)*
				if total == 0 {
					return;
				}
				let mut missing = ::std::vec::Vec::new();
				#(#coverage_checks)*
				assert!(
					missing.is_empty(),
					"Missing handlers for {} variants: {:?}",
					stringify!(#enum_name),
					missing
				);
			}
		}
	} else {
		quote! {}
	};

	let expanded = quote! {
		#[allow(non_upper_case_globals, missing_docs)]
		mod __dispatch_result_slices {
			use super::*;
			use ::linkme::distributed_slice;
			use crate::editor_ctx::ResultHandler;

			#(
				#[distributed_slice]
				pub static #slice_names: [ResultHandler];
			)*
		}

		pub use __dispatch_result_slices::*;

		impl #enum_name {
			/// Returns true if this result can be applied when a terminal is focused.
			pub fn is_terminal_safe(&self) -> bool {
				matches!(self, #(#terminal_safe_variants)|*)
			}
		}

		/// Dispatches an action result to its registered handlers.
		///
		/// Returns `true` if the editor should quit.
		pub fn dispatch_result(
			result: &#enum_name,
			ctx: &mut crate::editor_ctx::EditorContext,
			extend: bool,
		) -> bool {
			use crate::editor_ctx::HandleOutcome;
			use crate::editor_ctx::MessageAccess;

			fn run_handlers(
				handlers: &[crate::editor_ctx::ResultHandler],
				result: &#enum_name,
				ctx: &mut crate::editor_ctx::EditorContext,
				extend: bool,
			) -> HandleOutcome {
				let mut handlers = handlers.iter().collect::<Vec<_>>();
				handlers.sort_by_key(|handler| handler.priority);
				for handler in handlers {
					match (handler.handle)(result, ctx, extend) {
						HandleOutcome::Handled => return HandleOutcome::Handled,
						HandleOutcome::Quit => return HandleOutcome::Quit,
						HandleOutcome::NotHandled => continue,
					}
				}
				HandleOutcome::NotHandled
			}

			let mut handled = false;
			let outcome = match result {
				#(#match_arms,)*
			};

			match outcome {
				HandleOutcome::Quit => return true,
				HandleOutcome::Handled => handled = true,
				HandleOutcome::NotHandled => {}
			}

			let extension_outcome = run_handlers(&RESULT_EXTENSION_HANDLERS, result, ctx, extend);
			match extension_outcome {
				HandleOutcome::Quit => return true,
				HandleOutcome::Handled => handled = true,
				HandleOutcome::NotHandled => {}
			}

			if !handled {
				ctx.notify(
					"info",
					&format!(
						"Unhandled action result: {:?}",
						::std::mem::discriminant(result)
					),
				);
			}
			false
		}

		#coverage_test
	};

	expanded.into()
}

pub(crate) fn to_screaming_snake_case(s: &str) -> String {
	let mut result = String::new();
	for (i, c) in s.chars().enumerate() {
		if c.is_uppercase() && i > 0 {
			result.push('_');
		}
		result.push(c.to_ascii_uppercase());
	}
	result
}
