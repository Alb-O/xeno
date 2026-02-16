pub fn parse_overlay_new(working_set: &mut StateWorkingSet, call: Box<Call>) -> Pipeline {
	let call_span = call.span();

	let (overlay_name, _) = if let Some(expr) = call.positional_nth(0) {
		match eval_constant(working_set, expr) {
			Ok(val) => match val.coerce_into_string() {
				Ok(s) => (s, expr.span),
				Err(err) => {
					working_set.error(err.wrap(working_set, call_span));
					return garbage_pipeline(working_set, &[call_span]);
				}
			},
			Err(err) => {
				working_set.error(err.wrap(working_set, call_span));
				return garbage_pipeline(working_set, &[call_span]);
			}
		}
	} else {
		working_set.error(ParseError::UnknownState(
			"internal error: Missing required positional after call parsing".into(),
			call_span,
		));
		return garbage_pipeline(working_set, &[call_span]);
	};

	let pipeline = Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, Type::Any)]);

	let module_id = working_set.add_module(&overlay_name, Module::new(overlay_name.as_bytes().to_vec()), vec![]);

	working_set.add_overlay(
		overlay_name.as_bytes().to_vec(),
		module_id,
		ResolvedImportPattern::new(vec![], vec![], vec![], vec![]),
		false,
	);

	pipeline
}

pub fn parse_overlay_use(working_set: &mut StateWorkingSet, call: Box<Call>) -> Pipeline {
	let call_span = call.span();

	let (overlay_name, overlay_name_span) = if let Some(expr) = call.positional_nth(0) {
		match eval_constant(working_set, expr) {
			Ok(Value::Nothing { .. }) => {
				let mut call = call;
				call.set_parser_info("noop".to_string(), Expression::new_unknown(Expr::Bool(true), Span::unknown(), Type::Bool));
				return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, Type::Any)]);
			}
			Ok(val) => match val.coerce_into_string() {
				Ok(s) => (s, expr.span),
				Err(err) => {
					working_set.error(err.wrap(working_set, call_span));
					return garbage_pipeline(working_set, &[call_span]);
				}
			},
			Err(err) => {
				working_set.error(err.wrap(working_set, call_span));
				return garbage_pipeline(working_set, &[call_span]);
			}
		}
	} else {
		working_set.error(ParseError::UnknownState(
			"internal error: Missing required positional after call parsing".into(),
			call_span,
		));
		return garbage_pipeline(working_set, &[call_span]);
	};

	let new_name = if let Some(kw_expression) = call.positional_nth(1) {
		if let Some(new_name_expression) = kw_expression.as_keyword() {
			match eval_constant(working_set, new_name_expression) {
				Ok(val) => match val.coerce_into_string() {
					Ok(s) => Some(Spanned {
						item: s,
						span: new_name_expression.span,
					}),
					Err(err) => {
						working_set.error(err.wrap(working_set, call_span));
						return garbage_pipeline(working_set, &[call_span]);
					}
				},
				Err(err) => {
					working_set.error(err.wrap(working_set, call_span));
					return garbage_pipeline(working_set, &[call_span]);
				}
			}
		} else {
			working_set.error(ParseError::ExpectedKeyword("as keyword".to_string(), kw_expression.span));
			return garbage_pipeline(working_set, &[call_span]);
		}
	} else {
		None
	};

	let Ok(has_prefix) = has_flag_const(working_set, &call, "prefix") else {
		return garbage_pipeline(working_set, &[call_span]);
	};
	let Ok(do_reload) = has_flag_const(working_set, &call, "reload") else {
		return garbage_pipeline(working_set, &[call_span]);
	};

	let pipeline = Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call.clone()), call_span, Type::Any)]);

	let (final_overlay_name, origin_module, origin_module_id, is_module_updated) =
		if let Some(overlay_frame) = working_set.find_overlay(overlay_name.as_bytes()) {
			// Activate existing overlay

			// First, check for errors
			if has_prefix && !overlay_frame.prefixed {
				working_set.error(ParseError::OverlayPrefixMismatch(overlay_name, "without".to_string(), overlay_name_span));
				return pipeline;
			}

			if !has_prefix && overlay_frame.prefixed {
				working_set.error(ParseError::OverlayPrefixMismatch(overlay_name, "with".to_string(), overlay_name_span));
				return pipeline;
			}

			if let Some(new_name) = new_name
				&& new_name.item != overlay_name
			{
				working_set.error(ParseError::CantAddOverlayHelp(
					format!(
						"Cannot add overlay as '{}' because it already exists under the name '{}'",
						new_name.item, overlay_name
					),
					new_name.span,
				));
				return pipeline;
			}

			let module_id = overlay_frame.origin;

			if let Some(new_module_id) = working_set.find_module(overlay_name.as_bytes()) {
				if !do_reload && (module_id == new_module_id) {
					(overlay_name, Module::new(working_set.get_module(module_id).name.clone()), module_id, false)
				} else {
					// The origin module of an overlay changed => update it
					(overlay_name, working_set.get_module(new_module_id).clone(), new_module_id, true)
				}
			} else {
				let module_name = overlay_name.as_bytes().to_vec();
				(overlay_name, Module::new(module_name), module_id, true)
			}
		} else {
			// Create a new overlay
			if let Some(module_id) =
				// the name is a module
				working_set.find_module(overlay_name.as_bytes())
			{
				(
					new_name.map(|spanned| spanned.item).unwrap_or(overlay_name),
					working_set.get_module(module_id).clone(),
					module_id,
					true,
				)
			} else if let Some(module_id) = parse_module_file_or_dir(
				working_set,
				overlay_name.as_bytes(),
				overlay_name_span,
				new_name.as_ref().map(|spanned| spanned.item.clone()),
			) {
				// try file or directory
				let new_module = working_set.get_module(module_id).clone();
				(
					new_name
						.map(|spanned| spanned.item)
						.unwrap_or_else(|| String::from_utf8_lossy(&new_module.name).to_string()),
					new_module,
					module_id,
					true,
				)
			} else {
				working_set.error(ParseError::ModuleOrOverlayNotFound(overlay_name_span));
				return pipeline;
			}
		};

	let (definitions, errors) = if is_module_updated {
		if has_prefix {
			origin_module.resolve_import_pattern(working_set, origin_module_id, &[], Some(final_overlay_name.as_bytes()), call.head, &mut vec![])
		} else {
			origin_module.resolve_import_pattern(
				working_set,
				origin_module_id,
				&[ImportPatternMember::Glob { span: overlay_name_span }],
				Some(final_overlay_name.as_bytes()),
				call.head,
				&mut vec![],
			)
		}
	} else {
		(ResolvedImportPattern::new(vec![], vec![], vec![], vec![]), vec![])
	};

	if errors.is_empty() {
		working_set.add_overlay(final_overlay_name.as_bytes().to_vec(), origin_module_id, definitions, has_prefix);
	} else {
		working_set.parse_errors.extend(errors);
	}

	// Change the call argument to include the Overlay expression with the module ID
	let mut call = call;
	call.set_parser_info(
		"overlay_expr".to_string(),
		Expression::new(
			working_set,
			Expr::Overlay(if is_module_updated { Some(origin_module_id) } else { None }),
			overlay_name_span,
			Type::Any,
		),
	);

	Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, Type::Any)])
}

pub fn parse_overlay_hide(working_set: &mut StateWorkingSet, call: Box<Call>) -> Pipeline {
	let call_span = call.span();

	let (overlay_name, overlay_name_span) = if let Some(expr) = call.positional_nth(0) {
		match eval_constant(working_set, expr) {
			Ok(val) => match val.coerce_into_string() {
				Ok(s) => (s, expr.span),
				Err(err) => {
					working_set.error(err.wrap(working_set, call_span));
					return garbage_pipeline(working_set, &[call_span]);
				}
			},
			Err(err) => {
				working_set.error(err.wrap(working_set, call_span));
				return garbage_pipeline(working_set, &[call_span]);
			}
		}
	} else {
		(String::from_utf8_lossy(working_set.last_overlay_name()).to_string(), call_span)
	};

	let Ok(keep_custom) = has_flag_const(working_set, &call, "keep-custom") else {
		return garbage_pipeline(working_set, &[call_span]);
	};

	let pipeline = Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, Type::Any)]);

	if overlay_name == DEFAULT_OVERLAY_NAME {
		working_set.error(ParseError::CantHideDefaultOverlay(overlay_name, overlay_name_span));

		return pipeline;
	}

	if !working_set.unique_overlay_names().contains(&overlay_name.as_bytes()) {
		working_set.error(ParseError::ActiveOverlayNotFound(overlay_name_span));
		return pipeline;
	}

	if working_set.num_overlays() < 2 {
		working_set.error(ParseError::CantRemoveLastOverlay(overlay_name_span));
		return pipeline;
	}

	working_set.remove_overlay(overlay_name.as_bytes(), keep_custom);

	pipeline
}

pub fn parse_let(working_set: &mut StateWorkingSet, spans: &[Span]) -> Pipeline {
	trace!("parsing: let");

	// JT: Disabling check_name because it doesn't work with optional types in the declaration
	// if let Some(span) = check_name(working_set, spans) {
	//     return Pipeline::from_vec(vec![garbage(*span)]);
	// }

	if let Some(decl_id) = working_set.find_decl(b"let") {
		if spans.len() >= 4 {
			// This is a bit of by-hand parsing to get around the issue where we want to parse in the reverse order
			// so that the var-id created by the variable isn't visible in the expression that init it
			for span in spans.iter().enumerate() {
				let item = working_set.get_span_contents(*span.1);
				// https://github.com/nushell/nushell/issues/9596, let = if $
				// let x = 'f', = at least start from index 2
				if item == b"=" && spans.len() > (span.0 + 1) && span.0 > 1 {
					let (tokens, parse_error) = lex(
						working_set.get_span_contents(Span::concat(&spans[(span.0 + 1)..])),
						spans[span.0 + 1].start,
						&[],
						&[],
						false,
					);

					if let Some(parse_error) = parse_error {
						working_set.error(parse_error)
					}

					let rvalue_span = Span::concat(&spans[(span.0 + 1)..]);
					let rvalue_block = parse_block(working_set, &tokens, rvalue_span, false, true);

					let output_type = rvalue_block.output_type();

					let block_id = working_set.add_block(Arc::new(rvalue_block));

					let rvalue = Expression::new(working_set, Expr::Block(block_id), rvalue_span, output_type);

					let mut idx = 0;
					let (lvalue, explicit_type) = parse_var_with_opt_type(working_set, &spans[1..(span.0)], &mut idx, false);
					// check for extra tokens after the identifier
					if idx + 1 < span.0 - 1 {
						working_set.error(ParseError::ExtraTokens(spans[idx + 2]));
					}

					ensure_not_reserved_variable_name(working_set, &lvalue);

					let var_id = lvalue.as_var();
					let rhs_type = rvalue.ty.clone();

					if let Some(explicit_type) = &explicit_type
						&& !type_compatible(explicit_type, &rhs_type)
					{
						working_set.error(ParseError::TypeMismatch(
							explicit_type.clone(),
							rhs_type.clone(),
							Span::concat(&spans[(span.0 + 1)..]),
						));
					}

					if let Some(var_id) = var_id
						&& explicit_type.is_none()
					{
						working_set.set_variable_type(var_id, rhs_type);
					}

					let call = Box::new(Call {
						decl_id,
						head: spans[0],
						arguments: vec![Argument::Positional(lvalue), Argument::Positional(rvalue)],
						parser_info: HashMap::new(),
					});

					return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)]);
				}
			}
		}
		let ParsedInternalCall { call, output, .. } = parse_internal_call(working_set, spans[0], &spans[1..], decl_id, ArgumentParsingLevel::Full);

		return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), output)]);
	} else {
		working_set.error(ParseError::UnknownState(
			"internal error: let or const statements not found in core language".into(),
			Span::concat(spans),
		))
	}

	working_set.error(ParseError::UnknownState(
		"internal error: let or const statement unparsable".into(),
		Span::concat(spans),
	));

	garbage_pipeline(working_set, spans)
}

/// Additionally returns a span encompassing the variable name, if successful.
pub fn parse_const(working_set: &mut StateWorkingSet, spans: &[Span]) -> (Pipeline, Option<Span>) {
	trace!("parsing: const");

	// JT: Disabling check_name because it doesn't work with optional types in the declaration
	// if let Some(span) = check_name(working_set, spans) {
	//     return Pipeline::from_vec(vec![garbage(working_set, *span)]);
	// }

	if let Some(decl_id) = working_set.find_decl(b"const") {
		if spans.len() >= 4 {
			// This is a bit of by-hand parsing to get around the issue where we want to parse in the reverse order
			// so that the var-id created by the variable isn't visible in the expression that init it
			for span in spans.iter().enumerate() {
				let item = working_set.get_span_contents(*span.1);
				// const x = 'f', = at least start from index 2
				if item == b"=" && spans.len() > (span.0 + 1) && span.0 > 1 {
					// Parse the rvalue as a subexpression
					let rvalue_span = Span::concat(&spans[(span.0 + 1)..]);

					let (rvalue_tokens, rvalue_error) = lex(working_set.get_span_contents(rvalue_span), rvalue_span.start, &[], &[], false);
					working_set.parse_errors.extend(rvalue_error);

					trace!("parsing: const right-hand side subexpression");
					let rvalue_block = parse_block(working_set, &rvalue_tokens, rvalue_span, false, true);
					let rvalue_ty = rvalue_block.output_type();
					let rvalue_block_id = working_set.add_block(Arc::new(rvalue_block));
					let rvalue = Expression::new(working_set, Expr::Subexpression(rvalue_block_id), rvalue_span, rvalue_ty);

					let mut idx = 0;

					let (lvalue, explicit_type) = parse_var_with_opt_type(working_set, &spans[1..(span.0)], &mut idx, false);
					// check for extra tokens after the identifier
					if idx + 1 < span.0 - 1 {
						working_set.error(ParseError::ExtraTokens(spans[idx + 2]));
					}

					ensure_not_reserved_variable_name(working_set, &lvalue);

					let var_id = lvalue.as_var();
					let rhs_type = rvalue.ty.clone();

					if let Some(explicit_type) = &explicit_type
						&& !type_compatible(explicit_type, &rhs_type)
					{
						working_set.error(ParseError::TypeMismatch(
							explicit_type.clone(),
							rhs_type.clone(),
							Span::concat(&spans[(span.0 + 1)..]),
						));
					}

					if let Some(var_id) = var_id {
						if explicit_type.is_none() {
							working_set.set_variable_type(var_id, rhs_type);
						}

						match eval_constant(working_set, &rvalue) {
							Ok(mut value) => {
								// In case rhs is parsed as 'any' but is evaluated to a concrete
								// type:
								let mut const_type = value.get_type();

								if let Some(explicit_type) = &explicit_type {
									if !type_compatible(explicit_type, &const_type) {
										working_set.error(ParseError::TypeMismatch(
											explicit_type.clone(),
											const_type.clone(),
											Span::concat(&spans[(span.0 + 1)..]),
										));
									}
									let val_span = value.span();

									// need to convert to Value::glob if rhs is string, and
									// the const variable is annotated with glob type.
									match value {
										Value::String { val, .. } if explicit_type == &Type::Glob => {
											value = Value::glob(val, false, val_span);
											const_type = value.get_type();
										}
										_ => {}
									}
								}

								working_set.set_variable_type(var_id, const_type);

								// Assign the constant value to the variable
								working_set.set_variable_const_val(var_id, value);
							}
							Err(err) => working_set.error(err.wrap(working_set, rvalue.span)),
						}
					}

					let call = Box::new(Call {
						decl_id,
						head: spans[0],
						arguments: vec![Argument::Positional(lvalue.clone()), Argument::Positional(rvalue)],
						parser_info: HashMap::new(),
					});

					return (
						Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)]),
						Some(lvalue.span),
					);
				}
			}
		}
		let ParsedInternalCall { call, output, .. } = parse_internal_call(working_set, spans[0], &spans[1..], decl_id, ArgumentParsingLevel::Full);

		return (
			Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), output)]),
			None,
		);
	} else {
		working_set.error(ParseError::UnknownState(
			"internal error: let or const statements not found in core language".into(),
			Span::concat(spans),
		))
	}

	working_set.error(ParseError::UnknownState(
		"internal error: let or const statement unparsable".into(),
		Span::concat(spans),
	));

	(garbage_pipeline(working_set, spans), None)
}

pub fn parse_mut(working_set: &mut StateWorkingSet, spans: &[Span]) -> Pipeline {
	trace!("parsing: mut");

	// JT: Disabling check_name because it doesn't work with optional types in the declaration
	// if let Some(span) = check_name(working_set, spans) {
	//     return Pipeline::from_vec(vec![garbage(working_set, *span)]);
	// }

	if let Some(decl_id) = working_set.find_decl(b"mut") {
		if spans.len() >= 4 {
			// This is a bit of by-hand parsing to get around the issue where we want to parse in the reverse order
			// so that the var-id created by the variable isn't visible in the expression that init it
			for span in spans.iter().enumerate() {
				let item = working_set.get_span_contents(*span.1);
				// mut x = 'f', = at least start from index 2
				if item == b"=" && spans.len() > (span.0 + 1) && span.0 > 1 {
					let (tokens, parse_error) = lex(
						working_set.get_span_contents(Span::concat(&spans[(span.0 + 1)..])),
						spans[span.0 + 1].start,
						&[],
						&[],
						false,
					);

					if let Some(parse_error) = parse_error {
						working_set.error(parse_error);
					}

					let rvalue_span = Span::concat(&spans[(span.0 + 1)..]);
					let rvalue_block = parse_block(working_set, &tokens, rvalue_span, false, true);

					let output_type = rvalue_block.output_type();

					let block_id = working_set.add_block(Arc::new(rvalue_block));

					let rvalue = Expression::new(working_set, Expr::Block(block_id), rvalue_span, output_type);

					let mut idx = 0;

					let (lvalue, explicit_type) = parse_var_with_opt_type(working_set, &spans[1..(span.0)], &mut idx, true);
					// check for extra tokens after the identifier
					if idx + 1 < span.0 - 1 {
						working_set.error(ParseError::ExtraTokens(spans[idx + 2]));
					}

					ensure_not_reserved_variable_name(working_set, &lvalue);

					let var_id = lvalue.as_var();
					let rhs_type = rvalue.ty.clone();

					if let Some(explicit_type) = &explicit_type
						&& !type_compatible(explicit_type, &rhs_type)
					{
						working_set.error(ParseError::TypeMismatch(
							explicit_type.clone(),
							rhs_type.clone(),
							Span::concat(&spans[(span.0 + 1)..]),
						));
					}

					if let Some(var_id) = var_id
						&& explicit_type.is_none()
					{
						working_set.set_variable_type(var_id, rhs_type);
					}

					let call = Box::new(Call {
						decl_id,
						head: spans[0],
						arguments: vec![Argument::Positional(lvalue), Argument::Positional(rvalue)],
						parser_info: HashMap::new(),
					});

					return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)]);
				}
			}
		}
		let ParsedInternalCall { call, output, .. } = parse_internal_call(working_set, spans[0], &spans[1..], decl_id, ArgumentParsingLevel::Full);

		return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), output)]);
	} else {
		working_set.error(ParseError::UnknownState(
			"internal error: let or const statements not found in core language".into(),
			Span::concat(spans),
		))
	}

	working_set.error(ParseError::UnknownState(
		"internal error: let or const statement unparsable".into(),
		Span::concat(spans),
	));

	garbage_pipeline(working_set, spans)
}

pub fn parse_source(working_set: &mut StateWorkingSet, lite_command: &LiteCommand) -> Pipeline {
	trace!("parsing source");
	let spans = &lite_command.parts;
	let name = working_set.get_span_contents(spans[0]);

	if name == b"source" || name == b"source-env" {
		if let Some(redirection) = lite_command.redirection.as_ref() {
			let name = if name == b"source" { "source" } else { "source-env" };
			working_set.error(redirecting_builtin_error(name, redirection));
			return garbage_pipeline(working_set, spans);
		}

		let scoped = name == b"source-env";

		if let Some(decl_id) = working_set.find_decl(name) {
			#[allow(deprecated)]
			let cwd = working_set.get_cwd();

			// Is this the right call to be using here?
			// Some of the others (`parse_let`) use it, some of them (`parse_hide`) don't.
			let ParsedInternalCall { call, output, call_kind } = parse_internal_call(working_set, spans[0], &spans[1..], decl_id, ArgumentParsingLevel::Full);

			if call_kind == CallKind::Help {
				return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), output)]);
			}

			// Command and one file name
			if let Some(expr) = call.positional_nth(0) {
				let val = match eval_constant(working_set, expr) {
					Ok(val) => val,
					Err(err) => {
						working_set.error(err.wrap(working_set, Span::concat(&spans[1..])));
						return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(&spans[1..]), Type::Any)]);
					}
				};

				if val.is_nothing() {
					let mut call = call;
					call.set_parser_info("noop".to_string(), Expression::new_unknown(Expr::Nothing, Span::unknown(), Type::Nothing));
					return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)]);
				}

				let filename = match val.coerce_into_string() {
					Ok(s) => s,
					Err(err) => {
						working_set.error(err.wrap(working_set, Span::concat(&spans[1..])));
						return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(&spans[1..]), Type::Any)]);
					}
				};

				if let Some(path) = find_in_dirs(&filename, working_set, &cwd, Some(LIB_DIRS_VAR)) {
					if let Some(contents) = path.read(working_set) {
						// Add the file to the stack of files being processed.
						if let Err(e) = working_set.files.push(path.clone().path_buf(), spans[1]) {
							working_set.error(e);
							return garbage_pipeline(working_set, spans);
						}

						// This will load the defs from the file into the
						// working set, if it was a successful parse.
						let mut block = parse(working_set, Some(&path.path().to_string_lossy()), &contents, scoped);
						if block.ir_block.is_none() {
							let block_mut = Arc::make_mut(&mut block);
							compile_block(working_set, block_mut);
						}

						// Remove the file from the stack of files being processed.
						working_set.files.pop();

						// Save the block into the working set
						let block_id = working_set.add_block(block);

						let mut call_with_block = call;

						// FIXME: Adding this expression to the positional creates a syntax highlighting error
						// after writing `source example.nu`
						call_with_block.set_parser_info(
							"block_id".to_string(),
							Expression::new(working_set, Expr::Int(block_id.get() as i64), spans[1], Type::Any),
						);

						// store the file path as a string to be gathered later
						call_with_block.set_parser_info(
							"block_id_name".to_string(),
							Expression::new(
								working_set,
								Expr::Filepath(path.path_buf().display().to_string(), false),
								spans[1],
								Type::String,
							),
						);

						return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call_with_block), Span::concat(spans), Type::Any)]);
					}
				} else {
					working_set.error(ParseError::SourcedFileNotFound(filename, spans[1]));
				}
			}
			return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)]);
		}
	}
	working_set.error(ParseError::UnknownState(
		"internal error: source statement unparsable".into(),
		Span::concat(spans),
	));
	garbage_pipeline(working_set, spans)
}

pub fn parse_where_expr(working_set: &mut StateWorkingSet, spans: &[Span]) -> Expression {
	trace!("parsing: where");

	if !spans.is_empty() && working_set.get_span_contents(spans[0]) != b"where" {
		working_set.error(ParseError::UnknownState(
			"internal error: Wrong call name for 'where' command".into(),
			Span::concat(spans),
		));
		return garbage(working_set, Span::concat(spans));
	}

	if spans.len() < 2 {
		working_set.error(ParseError::MissingPositional(
			"row condition".into(),
			Span::concat(spans),
			"where <row_condition>".into(),
		));
		return garbage(working_set, Span::concat(spans));
	}

	let call = match working_set.find_decl(b"where") {
		Some(decl_id) => {
			let ParsedInternalCall { call, output, call_kind } = parse_internal_call(working_set, spans[0], &spans[1..], decl_id, ArgumentParsingLevel::Full);

			if call_kind != CallKind::Valid {
				return Expression::new(working_set, Expr::Call(call), Span::concat(spans), output);
			}

			call
		}
		None => {
			working_set.error(ParseError::UnknownState(
				"internal error: 'where' declaration not found".into(),
				Span::concat(spans),
			));
			return garbage(working_set, Span::concat(spans));
		}
	};

	Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)
}

pub fn parse_where(working_set: &mut StateWorkingSet, lite_command: &LiteCommand) -> Pipeline {
	let expr = parse_where_expr(working_set, &lite_command.parts);
	let redirection = lite_command.redirection.as_ref().map(|r| parse_redirection(working_set, r));

	let element = PipelineElement { pipe: None, expr, redirection };

	Pipeline { elements: vec![element] }
}

pub fn find_dirs_var(working_set: &StateWorkingSet, var_name: &str) -> Option<VarId> {
	working_set
		.find_variable(format!("${var_name}").as_bytes())
		.filter(|var_id| working_set.get_variable(*var_id).const_val.is_some())
}

/// This helper function is used to find files during parsing
///
/// First, the actual current working directory is selected as
///   a) the directory of a file currently being parsed
///   b) current working directory (PWD)
///
/// Then, if the file is not found in the actual cwd, dirs_var is checked.
/// For now, we first check for a const with the name of `dirs_var_name`,
/// and if that's not found, then we try to look for an environment variable of the same name.
/// If there is a relative path in dirs_var, it is assumed to be relative to the actual cwd
/// determined in the first step.
///
/// Always returns an absolute path
pub fn find_in_dirs(filename: &str, working_set: &StateWorkingSet, cwd: &str, dirs_var_name: Option<&str>) -> Option<ParserPath> {
	if is_windows_device_path(Path::new(&filename)) {
		return Some(ParserPath::RealPath(filename.into()));
	}

	pub fn find_in_dirs_with_id(filename: &str, working_set: &StateWorkingSet, cwd: &str, dirs_var_name: Option<&str>) -> Option<ParserPath> {
		// Choose whether to use file-relative or PWD-relative path
		let actual_cwd = working_set.files.current_working_directory().unwrap_or(Path::new(cwd));

		// Try if we have an existing virtual path
		if let Some(virtual_path) = working_set.find_virtual_path(filename) {
			return Some(ParserPath::from_virtual_path(working_set, filename, virtual_path));
		} else {
			let abs_virtual_filename = actual_cwd.join(filename);
			let abs_virtual_filename = abs_virtual_filename.to_string_lossy();

			if let Some(virtual_path) = working_set.find_virtual_path(&abs_virtual_filename) {
				return Some(ParserPath::from_virtual_path(working_set, &abs_virtual_filename, virtual_path));
			}
		}

		// Try if we have an existing filesystem path
		if let Ok(p) = absolute_with(filename, actual_cwd)
			&& p.exists()
		{
			return Some(ParserPath::RealPath(p));
		}

		// Early-exit if path is non-existent absolute path
		let path = Path::new(filename);
		if !path.is_relative() {
			return None;
		}

		// Look up relative path from NU_LIB_DIRS
		dirs_var_name
			.as_ref()
			.and_then(|dirs_var_name| find_dirs_var(working_set, dirs_var_name))
			.map(|var_id| working_set.get_variable(var_id))?
			.const_val
			.as_ref()?
			.as_list()
			.ok()?
			.iter()
			.map(|lib_dir| -> Option<PathBuf> {
				let dir = lib_dir.to_path().ok()?;
				let dir_abs = absolute_with(dir, actual_cwd).ok()?;
				let path = absolute_with(filename, dir_abs).ok()?;
				path.exists().then_some(path)
			})
			.find(Option::is_some)
			.flatten()
			.map(ParserPath::RealPath)
	}

	// TODO: remove (see #8310)
	// Same as find_in_dirs_with_id but using $env.NU_LIB_DIRS instead of constant
	pub fn find_in_dirs_old(filename: &str, working_set: &StateWorkingSet, cwd: &str, dirs_env: Option<&str>) -> Option<PathBuf> {
		// Choose whether to use file-relative or PWD-relative path
		let actual_cwd = working_set.files.current_working_directory().unwrap_or(Path::new(cwd));

		if let Ok(p) = absolute_with(filename, actual_cwd)
			&& p.exists()
		{
			Some(p)
		} else {
			let path = Path::new(filename);

			if path.is_relative() {
				if let Some(lib_dirs) = dirs_env.and_then(|dirs_env| working_set.get_env_var(dirs_env)) {
					if let Ok(dirs) = lib_dirs.as_list() {
						for lib_dir in dirs {
							if let Ok(dir) = lib_dir.to_path() {
								// make sure the dir is absolute path
								if let Ok(dir_abs) = absolute_with(dir, actual_cwd)
									&& let Ok(path) = absolute_with(filename, dir_abs)
									&& path.exists()
								{
									return Some(path);
								}
							}
						}

						None
					} else {
						None
					}
				} else {
					None
				}
			} else {
				None
			}
		}
	}

	find_in_dirs_with_id(filename, working_set, cwd, dirs_var_name)
		.or_else(|| find_in_dirs_old(filename, working_set, cwd, dirs_var_name).map(ParserPath::RealPath))
}

fn detect_params_in_name(working_set: &StateWorkingSet, name_span: Span, decl_id: DeclId) -> Option<ParseError> {
	let name = working_set.get_span_contents(name_span);
	for (offset, char) in name.iter().enumerate() {
		if *char == b'[' || *char == b'(' {
			return Some(ParseError::LabeledErrorWithHelp {
				error: "no space between name and parameters".into(),
				label: "expected space".into(),
				help: format!(
					"consider adding a space between the `{}` command's name and its parameters",
					working_set.get_decl(decl_id).name()
				),
				span: Span::new(offset + name_span.start - 1, offset + name_span.start - 1),
			});
		}
	}

	None
}

/// Run has_flag_const and push possible error to working_set
fn has_flag_const(working_set: &mut StateWorkingSet, call: &Call, name: &str) -> Result<bool, ()> {
	call.has_flag_const(working_set, name).map_err(|err| {
		working_set.error(err.wrap(working_set, call.span()));
	})
}
