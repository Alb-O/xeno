/// Evaluate a call
fn eval_call<D: DebugContext>(ctx: &mut EvalContext<'_>, decl_id: DeclId, head: Span, mut input: PipelineData) -> Result<PipelineData, ShellError> {
	let EvalContext {
		engine_state,
		stack: caller_stack,
		args_base,
		redirect_out,
		redirect_err,
		..
	} = ctx;

	let args_len = caller_stack.arguments.get_len(*args_base);
	let decl = engine_state.get_decl(decl_id);

	// Set up redirect modes
	let mut caller_stack = caller_stack.push_redirection(redirect_out.take(), redirect_err.take());

	let result = (|| {
		if let Some(block_id) = decl.block_id() {
			// If the decl is a custom command
			let block = engine_state.get_block(block_id);

			// check types after acquiring block to avoid unnecessarily cloning Signature
			check_input_types(&input, &block.signature, head)?;

			// Set up a callee stack with the captures and move arguments from the stack into variables
			let mut callee_stack = caller_stack.gather_captures(engine_state, &block.captures);

			gather_arguments(engine_state, block, &mut caller_stack, &mut callee_stack, *args_base, args_len, head)?;

			// Add one to the recursion count, so we don't recurse too deep. Stack overflows are not
			// recoverable in Rust.
			callee_stack.recursion_count += 1;

			let result = eval_block_with_early_return::<D>(engine_state, &mut callee_stack, block, input).map(|p| p.body);

			// Move environment variables back into the caller stack scope if requested to do so
			if block.redirect_env {
				redirect_env(engine_state, &mut caller_stack, &callee_stack);
			}

			result
		} else {
			check_input_types(&input, &decl.signature(), head)?;
			// FIXME: precalculate this and save it somewhere
			let span = Span::merge_many(std::iter::once(head).chain(caller_stack.arguments.get_args(*args_base, args_len).iter().flat_map(|arg| arg.span())));

			let call = Call {
				decl_id,
				head,
				span,
				args_base: *args_base,
				args_len,
			};

			// Make sure that iterating value itself can be interrupted.
			// e.g: 0..inf | to md
			if let PipelineData::Value(v, ..) = &mut input {
				v.inject_signals(engine_state);
			}
			// Run the call
			decl.run(engine_state, &mut caller_stack, &(&call).into(), input)
		}
	})();

	drop(caller_stack);

	// Important that this runs, to reset state post-call:
	ctx.stack.arguments.leave_frame(ctx.args_base);
	ctx.redirect_out = None;
	ctx.redirect_err = None;

	result
}

fn find_named_var_id(sig: &Signature, name: &[u8], short: &[u8], span: Span) -> Result<VarId, ShellError> {
	sig.named
		.iter()
		.find(|n| {
			if !n.long.is_empty() {
				n.long.as_bytes() == name
			} else {
				// It's possible to only have a short name and no long name
				n.short.is_some_and(|s| s.encode_utf8(&mut [0; 4]).as_bytes() == short)
			}
		})
		.ok_or_else(|| ShellError::IrEvalError {
			msg: format!("block does not have an argument named `{}`", String::from_utf8_lossy(name)),
			span: Some(span),
		})
		.and_then(|flag| expect_named_var_id(flag, span))
}

fn expect_named_var_id(arg: &Flag, span: Span) -> Result<VarId, ShellError> {
	arg.var_id.ok_or_else(|| ShellError::IrEvalError {
		msg: format!("block signature is missing var id for named arg `{}`", arg.long),
		span: Some(span),
	})
}

fn expect_positional_var_id(arg: &PositionalArg, span: Span) -> Result<VarId, ShellError> {
	arg.var_id.ok_or_else(|| ShellError::IrEvalError {
		msg: format!("block signature is missing var id for positional arg `{}`", arg.name),
		span: Some(span),
	})
}

/// Move arguments from the stack into variables for a custom command
fn gather_arguments(
	engine_state: &EngineState,
	block: &Block,
	caller_stack: &mut Stack,
	callee_stack: &mut Stack,
	args_base: usize,
	args_len: usize,
	call_head: Span,
) -> Result<(), ShellError> {
	let mut positional_iter = block
		.signature
		.required_positional
		.iter()
		.map(|p| (p, true))
		.chain(block.signature.optional_positional.iter().map(|p| (p, false)));

	// Arguments that didn't get consumed by required/optional
	let mut rest = vec![];
	let mut rest_span: Option<Span> = None;

	// If we encounter a spread, all further positionals should go to rest
	let mut always_spread = false;

	for arg in caller_stack.arguments.drain_args(args_base, args_len) {
		match arg {
			Argument::Positional { span, val, .. } => {
				// Don't check next positional arg if we encountered a spread previously
				let next = (!always_spread).then(|| positional_iter.next()).flatten();
				if let Some((positional_arg, required)) = next {
					let var_id = expect_positional_var_id(positional_arg, span)?;
					if required {
						// By checking the type of the bound variable rather than converting the
						// SyntaxShape here, we might be able to save some allocations and effort
						let variable = engine_state.get_var(var_id);
						check_type(&val, &variable.ty)?;
					}
					callee_stack.add_var(var_id, val);
				} else {
					rest_span = Some(rest_span.map_or(val.span(), |s| s.append(val.span())));
					rest.push(val);
				}
			}
			Argument::Spread { vals, span: spread_span, .. } => match vals {
				Value::List { vals, .. } => {
					rest.extend(vals);
					rest_span = Some(rest_span.map_or(spread_span, |s| s.append(spread_span)));
					always_spread = true;
				}
				Value::Nothing { .. } => {
					rest_span = Some(rest_span.map_or(spread_span, |s| s.append(spread_span)));
					always_spread = true;
				}
				Value::Error { error, .. } => return Err(*error),
				_ => return Err(ShellError::CannotSpreadAsList { span: vals.span() }),
			},
			Argument::Flag { data, name, short, span } => {
				let var_id = find_named_var_id(&block.signature, &data[name], &data[short], span)?;
				callee_stack.add_var(var_id, Value::bool(true, span))
			}
			Argument::Named {
				data, name, short, span, val, ..
			} => {
				let var_id = find_named_var_id(&block.signature, &data[name], &data[short], span)?;
				callee_stack.add_var(var_id, val)
			}
			Argument::ParserInfo { .. } => (),
		}
	}

	// Add the collected rest of the arguments if a spread argument exists
	if let Some(rest_arg) = &block.signature.rest_positional {
		let rest_span = rest_span.unwrap_or(call_head);
		let var_id = expect_positional_var_id(rest_arg, rest_span)?;
		callee_stack.add_var(var_id, Value::list(rest, rest_span));
	}

	// Check for arguments that haven't yet been set and set them to their defaults
	for (positional_arg, _) in positional_iter {
		let var_id = expect_positional_var_id(positional_arg, call_head)?;
		callee_stack.add_var(var_id, positional_arg.default_value.clone().unwrap_or(Value::nothing(call_head)));
	}

	for named_arg in &block.signature.named {
		if let Some(var_id) = named_arg.var_id {
			// For named arguments, we do this check by looking to see if the variable was set yet on
			// the stack. This assumes that the stack's variables was previously empty, but that's a
			// fair assumption for a brand new callee stack.
			if !callee_stack.vars.iter().any(|(id, _)| *id == var_id) {
				let val = if named_arg.arg.is_none() {
					Value::bool(false, call_head)
				} else if let Some(value) = &named_arg.default_value {
					value.clone()
				} else {
					Value::nothing(call_head)
				};
				callee_stack.add_var(var_id, val);
			}
		}
	}

	Ok(())
}

/// Type check helper. Produces `CantConvert` error if `val` is not compatible with `ty`.
fn check_type(val: &Value, ty: &Type) -> Result<(), ShellError> {
	match val {
		Value::Error { error, .. } => Err(*error.clone()),
		_ if val.is_subtype_of(ty) => Ok(()),
		_ => Err(ShellError::CantConvert {
			to_type: ty.to_string(),
			from_type: val.get_type().to_string(),
			span: val.span(),
			help: None,
		}),
	}
}

/// Type check and convert value for assignment.
fn check_assignment_type(val: Value, target_ty: &Type) -> Result<Value, ShellError> {
	match val {
		Value::Error { error, .. } => Err(*error),
		_ if val.is_subtype_of(target_ty) => Ok(val), // No conversion needed, but compatible
		_ => Err(ShellError::CantConvert {
			to_type: target_ty.to_string(),
			from_type: val.get_type().to_string(),
			span: val.span(),
			help: None,
		}),
	}
}

/// Type check pipeline input against command's input types
fn check_input_types(input: &PipelineData, signature: &Signature, head: Span) -> Result<(), ShellError> {
	let io_types = &signature.input_output_types;

	// If a command doesn't have any input/output types, then treat command input type as any
	if io_types.is_empty() {
		return Ok(());
	}

	// If a command only has a nothing input type, then allow any input data
	if io_types.iter().all(|(intype, _)| intype == &Type::Nothing) {
		return Ok(());
	}

	match input {
		// early return error directly if detected
		PipelineData::Value(Value::Error { error, .. }, ..) => return Err(*error.clone()),
		// bypass run-time typechecking for custom types
		PipelineData::Value(Value::Custom { .. }, ..) => return Ok(()),
		_ => (),
	}

	// Check if the input type is compatible with *any* of the command's possible input types
	if io_types.iter().any(|(command_type, _)| input.is_subtype_of(command_type)) {
		return Ok(());
	}

	let input_types: Vec<Type> = io_types.iter().map(|(input, _)| input.clone()).collect();
	let expected_string = combined_type_string(&input_types, "and");

	match (input, expected_string) {
		(PipelineData::Empty, _) => Err(ShellError::PipelineEmpty { dst_span: head }),
		(_, Some(expected_string)) => Err(ShellError::OnlySupportsThisInputType {
			exp_input_type: expected_string,
			wrong_type: input.get_type().to_string(),
			dst_span: head,
			src_span: input.span().unwrap_or(Span::unknown()),
		}),
		// expected_string didn't generate properly, so we can't show the proper error
		(_, None) => Err(ShellError::NushellFailed {
			msg: "Command input type strings is empty, despite being non-zero earlier".to_string(),
		}),
	}
}

/// Get variable from [`Stack`] or [`EngineState`]
fn get_var(ctx: &EvalContext<'_>, var_id: VarId, span: Span) -> Result<Value, ShellError> {
	match var_id {
		// $env
		ENV_VARIABLE_ID => {
			let env_vars = ctx.stack.get_env_vars(ctx.engine_state);
			let env_columns = env_vars.keys();
			let env_values = env_vars.values();

			let mut pairs = env_columns.map(|x| x.to_string()).zip(env_values.cloned()).collect::<Vec<(String, Value)>>();

			pairs.sort_by(|a, b| a.0.cmp(&b.0));

			Ok(Value::record(pairs.into_iter().collect(), span))
		}
		_ => ctx.stack.get_var(var_id, span).or_else(|err| {
			// $nu is handled by getting constant
			if let Some(const_val) = ctx.engine_state.get_constant(var_id).cloned() {
				Ok(const_val.with_span(span))
			} else {
				Err(err)
			}
		}),
	}
}

/// Get an environment variable, case-insensitively
fn get_env_var_case_insensitive<'a>(ctx: &'a mut EvalContext<'_>, key: &str) -> Option<&'a Value> {
	// Read scopes in order
	for overlays in ctx.stack.env_vars.iter().rev().chain(std::iter::once(&ctx.engine_state.env_vars)) {
		// Read overlays in order
		for overlay_name in ctx.stack.active_overlays.iter().rev() {
			let Some(map) = overlays.get(overlay_name) else {
				// Skip if overlay doesn't exist in this scope
				continue;
			};
			let hidden = ctx.stack.env_hidden.get(overlay_name);
			let is_hidden = |key: &str| hidden.is_some_and(|hidden| hidden.contains(key));

			if let Some(val) = map
				// Check for exact match
				.get(key)
				// Skip when encountering an overlay where the key is hidden
				.filter(|_| !is_hidden(key))
				.or_else(|| {
					// Check to see if it exists at all in the map, with a different case
					map.iter().find_map(|(k, v)| {
						// Again, skip something that's hidden
						(k.eq_ignore_case(key) && !is_hidden(k)).then_some(v)
					})
				}) {
				return Some(val);
			}
		}
	}
	// Not found
	None
}

/// Get the existing name of an environment variable, case-insensitively. This is used to implement
/// case preservation of environment variables, so that changing an environment variable that
/// already exists always uses the same case.
fn get_env_var_name_case_insensitive<'a>(ctx: &mut EvalContext<'_>, key: &'a str) -> Cow<'a, str> {
	// Read scopes in order
	ctx.stack
		.env_vars
		.iter()
		.rev()
		.chain(std::iter::once(&ctx.engine_state.env_vars))
		.flat_map(|overlays| {
			// Read overlays in order
			ctx.stack.active_overlays.iter().rev().filter_map(|name| overlays.get(name))
		})
		.find_map(|map| {
			// Use the hashmap first to try to be faster?
			if map.contains_key(key) {
				Some(Cow::Borrowed(key))
			} else {
				map.keys().find(|k| k.eq_ignore_case(key)).map(|k| {
					// it exists, but with a different case
					Cow::Owned(k.to_owned())
				})
			}
		})
		// didn't exist.
		.unwrap_or(Cow::Borrowed(key))
}
