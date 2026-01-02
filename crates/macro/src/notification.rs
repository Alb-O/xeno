//! Notification type registration macro.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Token};

pub(crate) struct NotificationInput {
	pub static_name: syn::Ident,
	pub id: syn::LitStr,
	pub fields: Vec<(syn::Ident, syn::Expr)>,
}

impl Parse for NotificationInput {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let static_name: syn::Ident = input.parse()?;
		input.parse::<Token![,]>()?;
		let id: syn::LitStr = input.parse()?;

		let mut fields = Vec::new();
		while !input.is_empty() {
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
			if input.is_empty() {
				break;
			}
			let name: syn::Ident = input.parse()?;
			input.parse::<Token![:]>()?;
			let val: syn::Expr = input.parse()?;
			fields.push((name, val));
		}

		Ok(NotificationInput {
			static_name,
			id,
			fields,
		})
	}
}

pub fn register_notification(input: TokenStream) -> TokenStream {
	let NotificationInput {
		static_name,
		id,
		fields,
	} = parse_macro_input!(input as NotificationInput);

	let mut level = quote! { evildoer_registry::notifications::Level::Info };
	let mut semantic = quote! { evildoer_manifest::SEMANTIC_INFO };
	let mut dismiss = quote! { evildoer_registry::notifications::AutoDismiss::default() };
	let mut icon = quote! { None };
	let mut animation = quote! { evildoer_registry::notifications::Animation::Fade };
	let mut timing = quote! {
		(
			evildoer_registry::notifications::Timing::Fixed(::std::time::Duration::from_millis(200)),
			evildoer_registry::notifications::Timing::Auto,
			evildoer_registry::notifications::Timing::Fixed(::std::time::Duration::from_millis(200)),
		)
	};

	for (name, val) in fields {
		match name.to_string().as_str() {
			"level" => level = quote! { #val },
			"semantic" => semantic = quote! { #val },
			"style" => semantic = quote! { #val }, // Alias for backward compat during migration
			"dismiss" => dismiss = quote! { #val },
			"icon" => icon = quote! { Some(#val) },
			"animation" => animation = quote! { #val },
			"timing" => timing = quote! { #val },
			_ => {
				return syn::Error::new(name.span(), "Unknown notification field")
					.to_compile_error()
					.into();
			}
		}
	}

	let helper_name = format_ident!("{}", id.value().replace(".", "_"));
	let trait_name = format_ident!("Notify{}Ext", static_name);

	let expanded = quote! {
		#[::linkme::distributed_slice(evildoer_registry::notifications::NOTIFICATION_TYPES)]
		pub static #static_name: evildoer_registry::notifications::NotificationTypeDef =
			evildoer_registry::notifications::NotificationTypeDef {
				id: #id,
				name: #id,
				level: #level,
				icon: #icon,
				semantic: #semantic,
				auto_dismiss: #dismiss,
				animation: #animation,
				timing: #timing,
				priority: 0,
				source: evildoer_registry::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};

		pub trait #trait_name: evildoer_manifest::editor_ctx::MessageAccess {
			fn #helper_name(&mut self, msg: &str) {
				self.notify(#id, msg);
			}
		}

		impl<T: evildoer_manifest::editor_ctx::MessageAccess + ?Sized> #trait_name for T {}
	};

	expanded.into()
}
