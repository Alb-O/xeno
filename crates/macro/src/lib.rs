use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{FnArg, ItemTrait, ReturnType, Token, TraitItem, parse_macro_input};

#[proc_macro_attribute]
pub fn tome_api(attr: TokenStream, item: TokenStream) -> TokenStream {
	let input = parse_macro_input!(item as ItemTrait);
	let trait_name = &input.ident;
	let trait_items = &input.items;

	// Parse context type from attribute (e.g. #[tome_api(ExtensionHostContext)])
	let context_type = if attr.is_empty() {
		None
	} else {
		Some(parse_macro_input!(attr as syn::Type))
	};

	let guest_methods = trait_items.iter().filter_map(|item| {
		if let TraitItem::Fn(method) = item {
			let sig = &method.sig;
			let name = &sig.ident;

			let inputs = &sig.inputs;
			let args: Vec<_> = inputs
				.iter()
				.skip(1)
				.filter_map(|arg| {
					if let FnArg::Typed(pat) = arg {
						Some((&pat.pat, &pat.ty))
					} else {
						None
					}
				})
				.collect();

			let arg_names: Vec<_> = args.iter().map(|(n, _)| n).collect();
			let arg_types: Vec<_> = args.iter().map(|(_, t)| t).collect();

			let host_fn_name = format_ident!("{}", name);

			let return_type = match &sig.output {
				ReturnType::Default => quote! { () },
				ReturnType::Type(_, ty) => quote! { #ty },
			};

			let has_args = !args.is_empty();
			let struct_def = if has_args {
				quote! {
					#[derive(serde::Serialize)]
					struct Input<'a> {
						#(#arg_names: &'a #arg_types),*
					}
				}
			} else {
				quote! {
					#[derive(serde::Serialize)]
					struct Input {}
				}
			};

			let struct_init = if has_args {
				quote! {
					let input = Input { #(#arg_names: &#arg_names),* };
				}
			} else {
				quote! {
					let input = Input {};
				}
			};

			Some(quote! {
				pub fn #name(#(#arg_names: #arg_types),*) -> #return_type {
					#[link(wasm_import_module = "host")]
					extern "C" {
						fn #host_fn_name(ptr: u64) -> u64;
					}

					#struct_def
					#struct_init

					let input_json = serde_json::to_vec(&input).expect("Failed to serialize input");
					let input_mem = extism_pdk::Memory::from_bytes(input_json).expect("Failed to allocate memory");

					let offset = unsafe { #host_fn_name(input_mem.offset()) };
					let output_mem = extism_pdk::Memory::find(offset).expect("Failed to find output memory");

					let output: #return_type = serde_json::from_slice(&output_mem.to_vec()).expect("Failed to deserialize output");
					output
				}
			})
		} else {
			None
		}
	});

	let host_code = if let Some(ctx_type) = context_type {
		let host_macro_items: Vec<_> = trait_items
			.iter()
			.filter_map(|item| {
				if let TraitItem::Fn(method) = item {
					let sig = &method.sig;
					let name = &sig.ident;
					let name_str = name.to_string();

					let inputs = &sig.inputs;
					let args: Vec<_> = inputs
						.iter()
						.skip(1)
						.filter_map(|arg| {
							if let FnArg::Typed(pat) = arg {
								Some((&pat.pat, &pat.ty))
							} else {
								None
							}
						})
						.collect();
					let arg_names: Vec<_> = args.iter().map(|(n, _)| n).collect();
					let arg_types: Vec<_> = args.iter().map(|(_, t)| t).collect();

					let is_mutable = if let Some(FnArg::Receiver(recv)) = inputs.first() {
						recv.mutability.is_some()
					} else {
						false
					};

					let ctx_binding = if is_mutable {
						quote! { let mut ctx }
					} else {
						quote! { let ctx }
					};

					let input_binding = if arg_names.is_empty() {
						quote! { let _input }
					} else {
						quote! { let input }
					};

					Some(quote! {
						extism::host_fn!(pub #name(user_data: #ctx_type; input_str: String) -> String {
						   #[derive(serde::Deserialize)]
						   struct Input {
							   #(#arg_names: #arg_types),*
						   }

						   #input_binding: Input = serde_json::from_str(&input_str)
							   .map_err(|e| extism::Error::msg(format!("Invalid input for {}: {}", #name_str, e)))?;

						   let locked = user_data.get().map_err(|e| extism::Error::msg(e.to_string()))?;
						   #ctx_binding = locked.lock().map_err(|e| extism::Error::msg(e.to_string()))?;

						   let result = ctx.#name(#(input.#arg_names),*);

						   let output_json = serde_json::to_string(&result)
								.map_err(|e| extism::Error::msg(format!("Failed to serialize output: {}", e)))?;
						   Ok(output_json)
						});
					})
				} else {
					None
				}
			})
			.collect();

		let host_function_list: Vec<_> = trait_items
			.iter()
			.filter_map(|item| {
				if let TraitItem::Fn(method) = item {
					let name = &method.sig.ident;
					let name_str = name.to_string();
					Some(quote! {
						extism::Function::new(
							#name_str,
							[extism::ValType::I64],
							[extism::ValType::I64],
							ctx.clone(),
							#name
						),
					})
				} else {
					None
				}
			})
			.collect();

		let host_function_list_fn_name = format_ident!(
			"create_{}_host_functions",
			trait_name.to_string().to_lowercase()
		);

		quote! {
			#[cfg(not(target_arch = "wasm32"))]
			pub mod host_impl {
				use super::*;
				#(#host_macro_items)*

				pub fn #host_function_list_fn_name(ctx: extism::UserData<#ctx_type>) -> Vec<extism::Function> {
					 vec![
						 #(#host_function_list)*
					 ]
				}
			}
			#[cfg(not(target_arch = "wasm32"))]
			pub use host_impl::#host_function_list_fn_name;
		}
	} else {
		quote! {}
	};

	quote! {
		#input

		#[cfg(any(target_arch = "wasm32", feature = "guest"))]
		pub mod host {
			use super::*;
			#(#guest_methods)*
		}

		#host_code
	}
	.into()
}

struct NotificationInput {
	static_name: syn::Ident,
	id: syn::LitStr,
	fields: Vec<(syn::Ident, syn::Expr)>,
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

#[proc_macro]
pub fn register_notification(input: TokenStream) -> TokenStream {
	let NotificationInput {
		static_name,
		id,
		fields,
	} = parse_macro_input!(input as NotificationInput);

	let mut level = quote! { tome_manifest::notifications::Level::Info };
	let mut semantic = quote! { tome_manifest::SEMANTIC_INFO };
	let mut dismiss = quote! { tome_manifest::notifications::AutoDismiss::default() };
	let mut icon = quote! { None };
	let mut animation = quote! { tome_manifest::notifications::Animation::Fade };
	let mut timing = quote! {
		(
			tome_manifest::notifications::Timing::Fixed(::std::time::Duration::from_millis(200)),
			tome_manifest::notifications::Timing::Auto,
			tome_manifest::notifications::Timing::Fixed(::std::time::Duration::from_millis(200)),
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
		#[::linkme::distributed_slice(tome_manifest::notifications::NOTIFICATION_TYPES)]
		pub static #static_name: tome_manifest::notifications::NotificationTypeDef =
			tome_manifest::notifications::NotificationTypeDef {
				id: #id,
				name: #id,
				level: #level,
				icon: #icon,
				semantic: #semantic,
				auto_dismiss: #dismiss,
				animation: #animation,
				timing: #timing,
				priority: 0,
				source: tome_manifest::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};

		pub trait #trait_name: tome_manifest::editor_ctx::MessageAccess {
			fn #helper_name(&mut self, msg: &str) {
				self.notify(#id, msg);
			}
		}

		impl<T: tome_manifest::editor_ctx::MessageAccess + ?Sized> #trait_name for T {}
	};

	expanded.into()
}
