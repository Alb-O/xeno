//! Option derive macro implementation.
//!
//! Provides `#[derive_option]` for registering configuration options.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Expr, Item, Lit, Meta, parse_macro_input};

/// Entry point for the `#[derive_option]` macro.
///
/// Transforms a static definition into an option registration:
///
/// ```ignore
/// #[derive_option]
/// #[option(kdl = "tab-width", scope = buffer)]
/// /// Number of spaces a tab character occupies.
/// pub static TAB_WIDTH: i64 = 4;
/// ```
///
/// Generates:
/// - `__OPT_TAB_WIDTH` static registered in `OPTIONS` slice
/// - `TAB_WIDTH` constant as `TypedOptionKey<i64>`
pub fn derive_option(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as Item);

	let Item::Static(item) = input else {
		return syn::Error::new_spanned(&input, "Option can only be derived for static items")
			.to_compile_error()
			.into();
	};

	let Some(option_attr) = item.attrs.iter().find(|a| a.path().is_ident("option")) else {
		return syn::Error::new_spanned(&item, "missing #[option(...)] attribute")
			.to_compile_error()
			.into();
	};

	let mut kdl_key: Option<String> = None;
	let mut scope: Option<syn::Ident> = None;
	let mut priority: Option<i16> = None;

	if let Err(e) = option_attr.parse_nested_meta(|meta| {
		if meta.path.is_ident("kdl") {
			let value: syn::LitStr = meta.value()?.parse()?;
			kdl_key = Some(value.value());
			Ok(())
		} else if meta.path.is_ident("scope") {
			let ident: syn::Ident = meta.value()?.parse()?;
			let ident_str = ident.to_string();
			if ident_str != "global" && ident_str != "buffer" {
				return Err(meta.error("scope must be 'global' or 'buffer'"));
			}
			scope = Some(ident);
			Ok(())
		} else if meta.path.is_ident("priority") {
			let lit: syn::LitInt = meta.value()?.parse()?;
			priority = Some(lit.base10_parse()?);
			Ok(())
		} else {
			Err(meta.error("unknown option attribute"))
		}
	}) {
		return e.to_compile_error().into();
	}

	let Some(kdl_key) = kdl_key else {
		return syn::Error::new_spanned(option_attr, "missing required 'kdl' attribute")
			.to_compile_error()
			.into();
	};

	let Some(scope_ident) = scope else {
		return syn::Error::new_spanned(option_attr, "missing required 'scope' attribute")
			.to_compile_error()
			.into();
	};

	let scope_variant = if scope_ident == "global" {
		format_ident!("Global")
	} else {
		format_ident!("Buffer")
	};

	let priority = priority.unwrap_or(0);

	let name = &item.ident;
	let name_str = name.to_string();

	let ty = &item.ty;
	let ty_str = quote!(#ty).to_string();

	let (option_type, value_wrapper, key_type): (_, _, syn::Type) = match ty_str.as_str() {
		"i64" => (format_ident!("Int"), format_ident!("Int"), syn::parse_quote!(i64)),
		"bool" => (format_ident!("Bool"), format_ident!("Bool"), syn::parse_quote!(bool)),
		"String" => (format_ident!("String"), format_ident!("String"), syn::parse_quote!(String)),
		"& 'static str" | "&'static str" => {
			(format_ident!("String"), format_ident!("String"), syn::parse_quote!(String))
		}
		_ => {
			return syn::Error::new_spanned(
				ty,
				format!("unsupported option type: {ty_str}. Supported: i64, bool, String, &'static str"),
			)
			.to_compile_error()
			.into();
		}
	};

	let default_expr = &item.expr;
	let default_value = if ty_str.contains("str") {
		quote! { (#default_expr).to_string() }
	} else {
		quote! { #default_expr }
	};

	let description = item
		.attrs
		.iter()
		.filter_map(|attr| {
			if !attr.path().is_ident("doc") {
				return None;
			}
			let Meta::NameValue(meta) = &attr.meta else {
				return None;
			};
			let Expr::Lit(expr) = &meta.value else {
				return None;
			};
			let Lit::Str(lit) = &expr.lit else {
				return None;
			};
			Some(lit.value().trim().to_string())
		})
		.collect::<Vec<_>>()
		.join(" ");

	let description = if description.is_empty() {
		name_str.clone()
	} else {
		description
	};

	let internal_static = format_ident!("__OPT_{}", name_str);
	let vis = &item.vis;
	let other_attrs: Vec<_> = item
		.attrs
		.iter()
		.filter(|a| !a.path().is_ident("option"))
		.collect();

	let expanded = quote! {
		#[allow(non_upper_case_globals)]
		#[::linkme::distributed_slice(::xeno_registry_options::OPTIONS)]
		static #internal_static: ::xeno_registry_options::OptionDef = ::xeno_registry_options::OptionDef {
			id: ::core::concat!(::core::env!("CARGO_PKG_NAME"), "::", ::core::stringify!(#name)),
			name: ::core::stringify!(#name),
			kdl_key: #kdl_key,
			description: #description,
			value_type: ::xeno_registry_options::OptionType::#option_type,
			default: || ::xeno_registry_options::OptionValue::#value_wrapper(#default_value),
			scope: ::xeno_registry_options::OptionScope::#scope_variant,
			priority: #priority,
			source: ::xeno_registry_options::RegistrySource::Crate(::core::env!("CARGO_PKG_NAME")),
			validator: None,
		};

		#(#other_attrs)*
		#[doc = concat!("Typed handle for the `", stringify!(#name), "` option.")]
		#vis const #name: ::xeno_registry_options::TypedOptionKey<#key_type> =
			::xeno_registry_options::TypedOptionKey::new(&#internal_static);
	};

	expanded.into()
}
