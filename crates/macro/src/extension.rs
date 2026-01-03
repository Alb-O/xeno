use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
	Attribute, ImplItem, ImplItemFn, ItemImpl, LitInt, LitStr, Token, Type, parse_macro_input,
};

use crate::dispatch;

/// Parsed arguments from `#[extension(id = "...", priority = N)]`.
struct ExtensionArgs {
	/// Unique identifier for the extension.
	id: LitStr,
	/// Sort/initialization priority.
	priority: i16,
}

impl Parse for ExtensionArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut id: Option<LitStr> = None;
		let mut priority: Option<i16> = None;

		while !input.is_empty() {
			let key: syn::Ident = input.parse()?;
			input.parse::<Token![=]>()?;

			match key.to_string().as_str() {
				"id" => {
					let val: LitStr = input.parse()?;
					id = Some(val);
				}
				"priority" => {
					let val: LitInt = input.parse()?;
					priority = Some(val.base10_parse()?);
				}
				other => {
					return Err(syn::Error::new_spanned(
						key,
						format!("unknown extension attribute key: {other}"),
					));
				}
			}

			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}

		let id = id.ok_or_else(|| syn::Error::new(input.span(), "missing id for #[extension]"))?;
		let priority = priority.unwrap_or(0);

		Ok(Self { id, priority })
	}
}

/// Parsed arguments from `#[render(priority = N)]`.
struct RenderArgs {
	/// Render callback priority (higher runs first).
	priority: i16,
}

impl Parse for RenderArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut priority: Option<i16> = None;
		while !input.is_empty() {
			let key: syn::Ident = input.parse()?;
			input.parse::<Token![=]>()?;
			match key.to_string().as_str() {
				"priority" => {
					let val: LitInt = input.parse()?;
					priority = Some(val.base10_parse()?);
				}
				other => {
					return Err(syn::Error::new_spanned(
						key,
						format!("unknown render attribute key: {other}"),
					));
				}
			}
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}
		Ok(Self {
			priority: priority.unwrap_or(0),
		})
	}
}

/// Parsed arguments from `#[command("name", aliases = [...], description = "...")]`.
struct CommandArgs {
	/// Primary command name.
	name: LitStr,
	/// Alternative names for the command.
	aliases: Vec<LitStr>,
	/// Human-readable description.
	description: Option<LitStr>,
}

impl Parse for CommandArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let name: LitStr = input.parse()?;
		let mut aliases = Vec::new();
		let mut description = None;

		if input.peek(Token![,]) {
			input.parse::<Token![,]>()?;
		}

		while !input.is_empty() {
			let key: syn::Ident = input.parse()?;
			input.parse::<Token![=]>()?;
			match key.to_string().as_str() {
				"aliases" => {
					let arr: syn::ExprArray = input.parse()?;
					for expr in arr.elems {
						match expr {
							syn::Expr::Lit(syn::ExprLit {
								lit: syn::Lit::Str(lit),
								..
							}) => aliases.push(lit),
							other => {
								return Err(syn::Error::new_spanned(
									other,
									"aliases must be string literals",
								));
							}
						}
					}
				}
				"description" => {
					let val: LitStr = input.parse()?;
					description = Some(val);
				}
				other => {
					return Err(syn::Error::new_spanned(
						key,
						format!("unknown command attribute key: {other}"),
					));
				}
			}

			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}

		Ok(Self {
			name,
			aliases,
			description,
		})
	}
}

/// Parsed arguments from `#[hook(event = EventName, priority = N)]`.
struct HookArgs {
	/// The hook event type to subscribe to.
	event: syn::Ident,
	/// Handler priority (higher runs first).
	priority: i16,
	/// Human-readable description.
	description: Option<LitStr>,
}

impl Parse for HookArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut event: Option<syn::Ident> = None;
		let mut priority: Option<i16> = None;
		let mut description: Option<LitStr> = None;

		while !input.is_empty() {
			let key: syn::Ident = input.parse()?;
			input.parse::<Token![=]>()?;
			match key.to_string().as_str() {
				"event" => {
					let val: syn::Ident = input.parse()?;
					event = Some(val);
				}
				"priority" => {
					let val: LitInt = input.parse()?;
					priority = Some(val.base10_parse()?);
				}
				"description" => {
					let val: LitStr = input.parse()?;
					description = Some(val);
				}
				other => {
					return Err(syn::Error::new_spanned(
						key,
						format!("unknown hook attribute key: {other}"),
					));
				}
			}

			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}

		let event = event.ok_or_else(|| syn::Error::new(input.span(), "missing hook event"))?;

		Ok(Self {
			event,
			priority: priority.unwrap_or(100),
			description,
		})
	}
}

/// Parsed `#[init]` method information.
struct InitMethod {
	/// Method name.
	ident: syn::Ident,
	/// Whether method takes `&mut ExtensionMap` parameter.
	takes_map: bool,
	/// Whether method returns `Self` (to insert into map).
	returns_state: bool,
}

/// Parsed `#[render]` method information.
struct RenderMethod {
	/// Method name.
	ident: syn::Ident,
	/// Render priority.
	priority: i16,
}

/// Parsed `#[command]` method information.
struct CommandMethod {
	/// Method name.
	ident: syn::Ident,
	/// Command configuration.
	args: CommandArgs,
	/// Whether the method is async.
	is_async: bool,
	/// Whether method returns `CommandResult`.
	returns_command_result: bool,
}

/// Parsed `#[hook]` method information.
struct HookMethod {
	/// Method name.
	ident: syn::Ident,
	/// Hook configuration.
	args: HookArgs,
}

/// Extracts and removes an attribute by name from a list.
fn extract_attr(attrs: &mut Vec<Attribute>, name: &str) -> Option<Attribute> {
	let mut found = None;
	attrs.retain(|attr| {
		if attr.path().is_ident(name) {
			found = Some(attr.clone());
			false
		} else {
			true
		}
	});
	found
}

/// Checks if a method takes `&mut ExtensionMap` as its only parameter.
fn takes_extension_map(method: &ImplItemFn) -> syn::Result<bool> {
	if method.sig.inputs.is_empty() {
		return Ok(false);
	}
	if method.sig.inputs.len() != 1 {
		return Err(syn::Error::new_spanned(
			&method.sig,
			"#[init] must take zero args or &mut ExtensionMap",
		));
	}
	let arg = method.sig.inputs.first().unwrap();
	let syn::FnArg::Typed(typed) = arg else {
		return Err(syn::Error::new_spanned(arg, "#[init] must not take self"));
	};
	let syn::Type::Reference(ty_ref) = &*typed.ty else {
		return Err(syn::Error::new_spanned(
			typed,
			"#[init] must take &mut ExtensionMap",
		));
	};
	if ty_ref.mutability.is_none() {
		return Err(syn::Error::new_spanned(
			typed,
			"#[init] must take &mut ExtensionMap",
		));
	}
	let syn::Type::Path(path) = &*ty_ref.elem else {
		return Err(syn::Error::new_spanned(
			typed,
			"#[init] must take &mut ExtensionMap",
		));
	};
	let last = path
		.path
		.segments
		.last()
		.ok_or_else(|| syn::Error::new_spanned(path, "invalid ExtensionMap type"))?;
	if last.ident != "ExtensionMap" {
		return Err(syn::Error::new_spanned(
			last,
			"#[init] must take &mut ExtensionMap",
		));
	}
	Ok(true)
}

/// Checks if a method returns `Self` or the state type.
fn returns_state(method: &ImplItemFn, state_ident: &syn::Ident) -> bool {
	let syn::ReturnType::Type(_, ty) = &method.sig.output else {
		return false;
	};
	let syn::Type::Path(path) = &**ty else {
		return false;
	};
	let Some(last) = path.path.segments.last() else {
		return false;
	};
	last.ident == "Self" || last.ident == *state_ident
}

/// Checks if a method returns `CommandResult`.
fn returns_command_result(method: &ImplItemFn) -> bool {
	let syn::ReturnType::Type(_, ty) = &method.sig.output else {
		return false;
	};
	let syn::Type::Path(path) = &**ty else {
		return false;
	};
	let Some(last) = path.path.segments.last() else {
		return false;
	};
	last.ident == "CommandResult"
}

/// Implements the `#[extension]` proc macro for extension registration.
pub fn extension(attr: TokenStream, item: TokenStream) -> TokenStream {
	let extension_args = parse_macro_input!(attr as ExtensionArgs);
	let mut impl_block = parse_macro_input!(item as ItemImpl);

	let state_ty = impl_block.self_ty.clone();
	let state_ident = match &*impl_block.self_ty {
		Type::Path(path) => path
			.path
			.segments
			.last()
			.map(|seg| seg.ident.clone())
			.ok_or_else(|| syn::Error::new_spanned(path, "invalid extension type"))
			.unwrap(),
		other => {
			return syn::Error::new_spanned(other, "#[extension] must be on an impl block")
				.to_compile_error()
				.into();
		}
	};

	let mut init_method: Option<InitMethod> = None;
	let mut render_methods = Vec::new();
	let mut command_methods = Vec::new();
	let mut hook_methods = Vec::new();

	for item in &mut impl_block.items {
		let ImplItem::Fn(method) = item else { continue };

		if let Some(attr) = extract_attr(&mut method.attrs, "init") {
			if init_method.is_some() {
				return syn::Error::new_spanned(attr, "duplicate #[init] method")
					.to_compile_error()
					.into();
			}
			let takes_map = match takes_extension_map(method) {
				Ok(val) => val,
				Err(err) => return err.to_compile_error().into(),
			};
			let returns_state = returns_state(method, &state_ident);
			init_method = Some(InitMethod {
				ident: method.sig.ident.clone(),
				takes_map,
				returns_state,
			});
			continue;
		}

		if let Some(attr) = extract_attr(&mut method.attrs, "render") {
			let args = if attr.meta.require_list().is_ok() {
				match attr.parse_args::<RenderArgs>() {
					Ok(args) => args,
					Err(err) => return err.to_compile_error().into(),
				}
			} else {
				RenderArgs { priority: 0 }
			};
			render_methods.push(RenderMethod {
				ident: method.sig.ident.clone(),
				priority: args.priority,
			});
			continue;
		}

		if let Some(attr) = extract_attr(&mut method.attrs, "command") {
			let args = match attr.parse_args::<CommandArgs>() {
				Ok(args) => args,
				Err(err) => return err.to_compile_error().into(),
			};
			let is_async = method.sig.asyncness.is_some();
			let returns_command_result = returns_command_result(method);
			command_methods.push(CommandMethod {
				ident: method.sig.ident.clone(),
				args,
				is_async,
				returns_command_result,
			});
			continue;
		}

		if let Some(attr) = extract_attr(&mut method.attrs, "hook") {
			let args = match attr.parse_args::<HookArgs>() {
				Ok(args) => args,
				Err(err) => return err.to_compile_error().into(),
			};
			hook_methods.push(HookMethod {
				ident: method.sig.ident.clone(),
				args,
			});
		}
	}

	let mut generated = Vec::new();

	if let Some(init) = init_method {
		let init_ident = init.ident;
		let init_call = if init.takes_map {
			quote! { #state_ty::#init_ident(map) }
		} else {
			quote! { #state_ty::#init_ident() }
		};
		let init_body = if init.returns_state {
			quote! { map.insert(#init_call); }
		} else {
			quote! { #init_call; }
		};

		let init_static = format_ident!(
			"EXTENSION_INIT_{}",
			dispatch::to_screaming_snake_case(&state_ident.to_string())
		);

		let id = &extension_args.id;
		let priority = extension_args.priority;
		generated.push(quote! {
			#[::linkme::distributed_slice(xeno_api::editor::extensions::EXTENSIONS)]
			static #init_static: xeno_api::editor::extensions::ExtensionInitDef =
				xeno_api::editor::extensions::ExtensionInitDef {
					id: #id,
					priority: #priority,
					init: |map| { #init_body },
				};
		});
	}

	for render in render_methods {
		let render_ident = render.ident;
		let render_static = format_ident!(
			"EXTENSION_RENDER_{}_{}",
			dispatch::to_screaming_snake_case(&state_ident.to_string()),
			dispatch::to_screaming_snake_case(&render_ident.to_string())
		);
		let priority = render.priority;
		generated.push(quote! {
			#[::linkme::distributed_slice(xeno_api::editor::extensions::RENDER_EXTENSIONS)]
			static #render_static: xeno_api::editor::extensions::ExtensionRenderDef =
				xeno_api::editor::extensions::ExtensionRenderDef {
					priority: #priority,
					update: |editor| {
						let state = {
							let state = editor.extensions.get_mut::<#state_ty>();
							state.map(|state| state as *mut #state_ty)
						};
						if let Some(state) = state {
							// SAFETY: state borrow ends before reusing editor.
							unsafe { (&mut *state).#render_ident(editor); }
						}
					},
				};
		});
	}

	for command in command_methods {
		let CommandMethod {
			ident,
			args: command_args,
			is_async,
			returns_command_result,
		} = command;
		let handler_name = format_ident!(
			"__ext_cmd_{}_{}",
			dispatch::to_screaming_snake_case(&state_ident.to_string()).to_lowercase(),
			ident
		);
		let command_ident = match syn::parse_str::<syn::Ident>(&command_args.name.value()) {
			Ok(ident) => ident,
			Err(_) => {
				return syn::Error::new_spanned(
					command_args.name,
					"command name must be a valid Rust identifier",
				)
				.to_compile_error()
				.into();
			}
		};
		let await_call = if is_async {
			quote! { .await }
		} else {
			quote! {}
		};
		let call = quote! { state.#ident(ctx)#await_call };
		let result_expr = if returns_command_result {
			quote! { #call.map(|_| xeno_registry::commands::CommandOutcome::Ok) }
		} else {
			quote! { #call }
		};
		let description = command_args
			.description
			.unwrap_or_else(|| LitStr::new(&format!("{} command", ident), ident.span()));
		let aliases = command_args.aliases;
		let aliases_tokens = if aliases.is_empty() {
			quote! {}
		} else {
			quote! { aliases: &[#(#aliases),*], }
		};
		let id = &extension_args.id;
		generated.push(quote! {
			fn #handler_name<'a>(
				ctx: &'a mut xeno_registry::commands::CommandContext<'a>,
			) -> futures::future::LocalBoxFuture<'a, Result<xeno_registry::commands::CommandOutcome, xeno_registry::commands::CommandError>> {
				Box::pin(async move {
					let editor = unsafe {
						&mut *(ctx.editor as *mut dyn xeno_registry::commands::CommandEditorOps
							as *mut xeno_api::editor::Editor)
					};
					let Some(state) = editor.extensions.get_mut::<#state_ty>() else {
						return Err(xeno_registry::commands::CommandError::Failed(format!(
							"{} extension not loaded",
							#id
						)));
					};
					#result_expr
				})
			}

			xeno_registry::commands::command!(#command_ident, {
				#aliases_tokens
				description: #description
			}, handler: #handler_name);
		});
	}

	for hook in hook_methods {
		let hook_ident = hook.ident;
		let hook_name = format_ident!(
			"{}_{}",
			dispatch::to_screaming_snake_case(&state_ident.to_string()).to_lowercase(),
			hook_ident
		);
		let event = hook.args.event;
		let priority = hook.args.priority;
		let description = hook
			.args
			.description
			.unwrap_or_else(|| LitStr::new(&format!("{} hook", hook_ident), hook_ident.span()));

		generated.push(quote! {
			xeno_registry::hook!(#hook_name, #event, #priority, #description, |ctx| {
				let Some(ext_map) = ctx.extensions::<xeno_api::editor::extensions::ExtensionMap>() else {
					return xeno_registry::hooks::HookAction::done();
				};
				let Some(state) = ext_map.get::<#state_ty>() else {
					return xeno_registry::hooks::HookAction::done();
				};
				let result = state.#hook_ident(ctx);
				::core::convert::Into::into(result)
			});
		});
	}

	let expanded = quote! {
		#impl_block
		#(#generated)*
	};

	expanded.into()
}
