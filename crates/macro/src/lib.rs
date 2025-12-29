use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Data, DeriveInput, FnArg, ItemTrait, ReturnType, Token, TraitItem, parse_macro_input};

#[proc_macro_attribute]
pub fn evildoer_api(attr: TokenStream, item: TokenStream) -> TokenStream {
	let input = parse_macro_input!(item as ItemTrait);
	let trait_name = &input.ident;
	let trait_items = &input.items;

	// Parse context type from attribute (e.g. #[evildoer_api(ExtensionHostContext)])
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

	let mut level = quote! { evildoer_manifest::notifications::Level::Info };
	let mut semantic = quote! { evildoer_manifest::SEMANTIC_INFO };
	let mut dismiss = quote! { evildoer_manifest::notifications::AutoDismiss::default() };
	let mut icon = quote! { None };
	let mut animation = quote! { evildoer_manifest::notifications::Animation::Fade };
	let mut timing = quote! {
		(
			evildoer_manifest::notifications::Timing::Fixed(::std::time::Duration::from_millis(200)),
			evildoer_manifest::notifications::Timing::Auto,
			evildoer_manifest::notifications::Timing::Fixed(::std::time::Duration::from_millis(200)),
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
		#[::linkme::distributed_slice(evildoer_manifest::notifications::NOTIFICATION_TYPES)]
		pub static #static_name: evildoer_manifest::notifications::NotificationTypeDef =
			evildoer_manifest::notifications::NotificationTypeDef {
				id: #id,
				name: #id,
				level: #level,
				icon: #icon,
				semantic: #semantic,
				auto_dismiss: #dismiss,
				animation: #animation,
				timing: #timing,
				priority: 0,
				source: evildoer_manifest::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
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

/// Derives dispatch infrastructure for `ActionResult`.
///
/// Generates:
/// - Handler slice declarations (`RESULT_*_HANDLERS`)
/// - `dispatch_result` function matching variants to handler slices
/// - `is_terminal_safe` method based on `#[terminal_safe]` attributes
///
/// # Attributes
///
/// - `#[handler(Foo)]` - Use `RESULT_FOO_HANDLERS` instead of deriving from variant name
/// - `#[terminal_safe]` - Mark variant as safe to execute when terminal is focused
///
/// # Example
///
/// ```ignore
/// #[derive(DispatchResult)]
/// pub enum ActionResult {
///     #[terminal_safe]
///     Ok,
///     #[terminal_safe]
///     #[handler(Quit)]
///     Quit,
///     #[terminal_safe]
///     #[handler(Quit)]
///     ForceQuit,
///     Motion(Selection),
/// }
/// ```
#[proc_macro_derive(DispatchResult, attributes(handler, terminal_safe))]
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

	for variant in &data.variants {
		let variant_name = &variant.ident;

		let handler_name = variant
			.attrs
			.iter()
			.find_map(|attr| {
				if attr.path().is_ident("handler") {
					attr.parse_args::<syn::Ident>().ok()
				} else {
					None
				}
			})
			.unwrap_or_else(|| variant_name.clone());

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

		let pattern = match &variant.fields {
			syn::Fields::Unit => quote! { #enum_name::#variant_name },
			syn::Fields::Unnamed(_) => quote! { #enum_name::#variant_name(..) },
			syn::Fields::Named(_) => quote! { #enum_name::#variant_name { .. } },
		};

		match_arms.push(quote! {
			#pattern => run_handlers(&#slice_ident, result, ctx, extend)
		});
	}

	let expanded = quote! {
		#[allow(non_upper_case_globals)]
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
			) -> bool {
				for handler in handlers {
					match (handler.handle)(result, ctx, extend) {
						HandleOutcome::Handled => return false,
						HandleOutcome::Quit => return true,
						HandleOutcome::NotHandled => continue,
					}
				}
				ctx.notify(
					"info",
					&format!(
						"Unhandled action result: {:?}",
						::std::mem::discriminant(result)
					),
				);
				false
			}

			match result {
				#(#match_arms,)*
			}
		}
	};

	expanded.into()
}

fn to_screaming_snake_case(s: &str) -> String {
	let mut result = String::new();
	for (i, c) in s.chars().enumerate() {
		if c.is_uppercase() && i > 0 {
			result.push('_');
		}
		result.push(c.to_ascii_uppercase());
	}
	result
}

/// Parses KDL keybinding definitions at compile time.
///
/// Takes a KDL string with mode blocks and key bindings, validates keys at compile time,
/// and generates `KeyBindingDef` statics.
///
/// # Input Format
///
/// ```kdl
/// normal "h" "left" "ctrl-h"
/// insert "left"
/// goto "h"
/// ```
///
/// Each mode name (normal, insert, goto, view, window, match, space) contains key strings
/// that will be bound to the action.
///
/// # Usage
///
/// This is called internally by `action!` macro:
///
/// ```ignore
/// action!(
///     move_left,
///     {
///         description: "Move cursor left",
///         bindings: r#"
///             normal "h" "left"
///             insert "left"
///         "#
///     },
///     |ctx| { ... }
/// );
/// ```
#[proc_macro]
pub fn parse_keybindings(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as ParseKeybindingsInput);

	match generate_keybindings(&input.action_name, &input.kdl_str) {
		Ok(tokens) => tokens.into(),
		Err(e) => syn::Error::new(input.kdl_span, e).to_compile_error().into(),
	}
}

struct ParseKeybindingsInput {
	action_name: String,
	kdl_str: String,
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
			"goto" => quote! { Goto },
			"view" => quote! { View },
			"window" => quote! { Window },
			"match" => quote! { Match },
			"space" => quote! { Space },
			other => {
				return Err(format!(
					"Unknown mode: {other}. Valid modes: normal, insert, goto, view, window, match, space"
				));
			}
		};

		let slice_ident = format_ident!("KEYBINDINGS_{}", mode_upper);

		for (idx, entry) in node.entries().iter().enumerate() {
			if entry.name().is_some() {
				continue;
			}

			let Some(key_str) = entry.value().as_string() else {
				continue;
			};

			let parsed = evildoer_keymap_parser::parse(key_str)
				.map_err(|e| format!("Invalid key \"{key_str}\": {e}"))?;

			let key_tokens = node_to_key_tokens(&parsed)?;

			let static_ident = format_ident!("KB_{}_{}__{}", action_upper, mode_upper, idx);

			statics.push(quote! {
				#[allow(non_upper_case_globals)]
				#[::linkme::distributed_slice(evildoer_manifest::keybindings::#slice_ident)]
				static #static_ident: evildoer_manifest::keybindings::KeyBindingDef =
					evildoer_manifest::keybindings::KeyBindingDef {
						mode: evildoer_manifest::keybindings::BindingMode::#mode_variant,
						key: #key_tokens,
						action: #action_name,
						priority: 100,
					};
			});
		}
	}

	Ok(quote! { #(#statics)* })
}

fn node_to_key_tokens(
	node: &evildoer_keymap_parser::Node,
) -> Result<proc_macro2::TokenStream, String> {
	use evildoer_keymap_parser::{Key as ParserKey, Modifier};

	let code_tokens = match &node.key {
		ParserKey::Char(c) => quote! { evildoer_base::key::KeyCode::Char(#c) },
		ParserKey::F(n) => quote! { evildoer_base::key::KeyCode::F(#n) },
		ParserKey::Group(g) => {
			return Err(format!(
				"Key groups (@{g:?}) not supported in compile-time bindings"
			));
		}
		key => {
			let variant = format_ident!("{}", format!("{key:?}"));
			quote! { evildoer_base::key::KeyCode::#variant }
		}
	};

	let ctrl = node.modifiers & (Modifier::Ctrl as u8) != 0;
	let alt = node.modifiers & (Modifier::Alt as u8) != 0;
	let shift = node.modifiers & (Modifier::Shift as u8) != 0;

	Ok(quote! {
		evildoer_base::key::Key {
			code: #code_tokens,
			modifiers: evildoer_base::key::Modifiers { ctrl: #ctrl, alt: #alt, shift: #shift },
		}
	})
}
