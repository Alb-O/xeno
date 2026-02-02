extern crate proc_macro;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Data, DeriveInput, Fields, Ident, ItemFn, ItemStruct, LitInt, Token, parse_macro_input};

#[proc_macro_attribute]
pub fn migration(args: TokenStream, item: TokenStream) -> TokenStream {
	let args = parse_macro_input!(args as MigrationArgs);

	let input_fn = parse_macro_input!(item as ItemFn);
	let fn_name = &input_fn.sig.ident;

	let item = &args.item;
	let from_version = &args.from_version;
	let to_version = &args.to_version;

	let expanded = quote! {
		#input_fn

		inventory::submit! {
			::helix_db::helix_engine::storage_core::version_info::TransitionSubmission(
				::helix_db::helix_engine::storage_core::version_info::Transition::new(
					stringify!(#item),
					#from_version,
					#to_version,
					#fn_name
				)
			)
		}
	};
	expanded.into()
}

#[proc_macro_attribute]
pub fn helix_node(_attr: TokenStream, input: TokenStream) -> TokenStream {
	let mut item_struct = parse_macro_input!(input as ItemStruct);

	match &mut item_struct.fields {
		Fields::Named(fields) => {
			// Check if 'id' already exists
			for field in &fields.named {
				if field.ident.as_ref().map(|i| i == "id").unwrap_or(false) {
					let msg = "struct already has an 'id' field";
					return syn::Error::new_spanned(field, msg)
						.to_compile_error()
						.into();
				}
			}

			// Prepend 'id: String' to the fields
			let id_field: syn::Field = syn::parse_quote! { id: String };
			fields.named.insert(0, id_field);
		}
		_ => {
			let msg = "helix_node only supports structs with named fields";
			return syn::Error::new_spanned(item_struct, msg)
				.to_compile_error()
				.into();
		}
	}

	quote! { #item_struct }.into()
}

#[proc_macro_derive(Traversable)]
pub fn traversable_derive(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let name = &input.ident;

	// Verify that the struct has an 'id' field
	let has_id_field = match &input.data {
		Data::Struct(data) => data
			.fields
			.iter()
			.any(|field| field.ident.as_ref().map(|i| i == "id").unwrap_or(false)),
		_ => false,
	};

	if !has_id_field {
		return TokenStream::from(quote! {
			compile_error!("Traversable can only be derived for structs with an 'id: &'a str' field");
		});
	}

	// Extract lifetime parameter if present
	let lifetime = if let Some(param) = input.generics.lifetimes().next() {
		let lifetime = &param.lifetime;
		quote! { #lifetime }
	} else {
		quote! { 'a }
	};

	let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

	let expanded = quote! {
		impl #impl_generics #name #ty_generics #where_clause {
			pub fn id(&self) -> &#lifetime str {
				self.id
			}
		}
	};

	TokenStream::from(expanded)
}

struct MigrationArgs {
	item: Ident,
	_comma: Token![,],
	from_version: LitInt,
	_arrow: Token![->],
	to_version: LitInt,
}

impl Parse for MigrationArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(MigrationArgs {
			item: input.parse()?,
			_comma: input.parse()?,
			from_version: input.parse()?,
			_arrow: input.parse()?,
			to_version: input.parse()?,
		})
	}
}
