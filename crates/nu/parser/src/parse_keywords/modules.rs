pub fn parse_alias(working_set: &mut StateWorkingSet, lite_command: &LiteCommand, module_name: Option<&[u8]>) -> Pipeline {
	let spans = &lite_command.parts;

	let (name_span, split_id) = if spans.len() > 1 && working_set.get_span_contents(spans[0]) == b"export" {
		(spans[1], 2)
	} else {
		(spans[0], 1)
	};

	let name = working_set.get_span_contents(name_span);

	if name != b"alias" {
		working_set.error(ParseError::InternalError("Alias statement unparsable".into(), Span::concat(spans)));
		return garbage_pipeline(working_set, spans);
	}
	if let Some(redirection) = lite_command.redirection.as_ref() {
		working_set.error(redirecting_builtin_error("alias", redirection));
		return garbage_pipeline(working_set, spans);
	}

	if let Some(span) = check_alias_name(working_set, spans) {
		return Pipeline::from_vec(vec![garbage(working_set, *span)]);
	}

	if let Some(decl_id) = working_set.find_decl(b"alias") {
		let (command_spans, rest_spans) = spans.split_at(split_id);

		let original_starting_error_count = working_set.parse_errors.len();

		let ParsedInternalCall {
			call: alias_call,
			output,
			call_kind,
		} = parse_internal_call(working_set, Span::concat(command_spans), rest_spans, decl_id, ArgumentParsingLevel::Full);

		working_set.parse_errors.truncate(original_starting_error_count);

		let alias_pipeline = Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(alias_call.clone()), Span::concat(spans), output)]);

		if call_kind == CallKind::Help {
			return alias_pipeline;
		}

		let Some(alias_name_expr) = alias_call.positional_nth(0) else {
			working_set.error(ParseError::UnknownState("Missing positional after call check".to_string(), Span::concat(spans)));
			return garbage_pipeline(working_set, spans);
		};

		let alias_name = if let Some(name) = alias_name_expr.as_string() {
			if name.contains('#') || name.contains('^') || name.parse::<bytesize::ByteSize>().is_ok() || name.parse::<f64>().is_ok() {
				working_set.error(ParseError::AliasNotValid(alias_name_expr.span));
				return garbage_pipeline(working_set, spans);
			} else {
				name
			}
		} else {
			working_set.error(ParseError::AliasNotValid(alias_name_expr.span));
			return garbage_pipeline(working_set, spans);
		};

		if spans.len() >= split_id + 3 {
			if let Some(mod_name) = module_name {
				if alias_name.as_bytes() == mod_name {
					working_set.error(ParseError::NamedAsModule("alias".to_string(), alias_name, "main".to_string(), spans[split_id]));

					return alias_pipeline;
				}

				if alias_name == "main" {
					working_set.error(ParseError::ExportMainAliasNotAllowed(spans[split_id]));
					return alias_pipeline;
				}
			}

			let _equals = working_set.get_span_contents(spans[split_id + 1]);

			let replacement_spans = &spans[(split_id + 2)..];
			let first_bytes = working_set.get_span_contents(replacement_spans[0]);

			if first_bytes != b"if" && first_bytes != b"match" && is_math_expression_like(working_set, replacement_spans[0]) {
				// TODO: Maybe we need to implement a Display trait for Expression?
				let starting_error_count = working_set.parse_errors.len();
				let expr = parse_expression(working_set, replacement_spans);
				working_set.parse_errors.truncate(starting_error_count);

				let msg = format!("{:?}", expr.expr);
				let msg_parts: Vec<&str> = msg.split('(').collect();

				working_set.error(ParseError::CantAliasExpression(msg_parts[0].to_string(), replacement_spans[0]));
				return alias_pipeline;
			}

			let starting_error_count = working_set.parse_errors.len();
			working_set.search_predecls = false;

			let expr = parse_call(working_set, replacement_spans, replacement_spans[0]);

			working_set.search_predecls = true;

			if starting_error_count != working_set.parse_errors.len()
				&& let Some(e) = working_set.parse_errors.get(starting_error_count)
			{
				if let ParseError::MissingPositional(..) | ParseError::MissingRequiredFlag(..) | ParseError::MissingFlagParam(..) = e {
					working_set.parse_errors.truncate(original_starting_error_count);
					// ignore missing required positional
				} else {
					return garbage_pipeline(working_set, replacement_spans);
				}
			}

			let (command, wrapped_call) = match expr {
				Expression {
					expr: Expr::Call(ref rhs_call),
					..
				} => {
					let cmd = working_set.get_decl(rhs_call.decl_id);

					if cmd.is_keyword() && !ALIASABLE_PARSER_KEYWORDS.contains(&cmd.name().as_bytes()) {
						working_set.error(ParseError::CantAliasKeyword(
							ALIASABLE_PARSER_KEYWORDS
								.iter()
								.map(|bytes| String::from_utf8_lossy(bytes).to_string())
								.collect::<Vec<String>>()
								.join(", "),
							rhs_call.head,
						));
						return alias_pipeline;
					}

					(Some(cmd.clone_box()), expr)
				}
				Expression {
					expr: Expr::ExternalCall(..), ..
				} => (None, expr),
				_ => {
					working_set.error(ParseError::InternalError("Parsed call not a call".into(), expr.span));
					return alias_pipeline;
				}
			};

			// Tries to build a useful description string
			let (description, extra_description) = match lite_command.comments.is_empty() {
				// First from comments, if any are present
				false => working_set.build_desc(&lite_command.comments),
				// Then from the command itself
				true => match alias_call.arguments.get(1) {
					Some(Argument::Positional(Expression { expr: Expr::Keyword(kw), .. })) => {
						let aliased = working_set.get_span_contents(kw.expr.span);
						(format!("Alias for `{}`", String::from_utf8_lossy(aliased)), String::new())
					}
					// Then with a default.
					_ => ("User declared alias".into(), String::new()),
				},
			};

			let decl = Alias {
				name: alias_name,
				command,
				wrapped_call,
				description,
				extra_description,
			};

			working_set.add_decl(Box::new(decl));
		}

		// special case for `alias foo=bar`
		if spans.len() == 2 && working_set.get_span_contents(spans[1]).contains(&b'=') {
			let arg = String::from_utf8_lossy(working_set.get_span_contents(spans[1]));

			// split at '='.  Note that the output must never be None, the
			// `unwrap` is just to avoid the possibility of panic, if the
			// invariant is broken.
			let (name, initial_value) = arg.split_once('=').unwrap_or((&arg, ""));

			let name = if name.is_empty() { "{name}" } else { name };
			let initial_value = if initial_value.is_empty() { "{initial_value}" } else { initial_value };

			working_set.error(ParseError::IncorrectValue(
				"alias argument".into(),
				spans[1],
				format!("Make sure to put spaces around '=': alias {name} = {initial_value}"),
			))
		} else if spans.len() < 4 {
			working_set.error(ParseError::IncorrectValue(
				"Incomplete alias".into(),
				Span::concat(&spans[..split_id]),
				"incomplete alias".into(),
			));
		}

		return alias_pipeline;
	}

	working_set.error(ParseError::InternalError("Alias statement unparsable".into(), Span::concat(spans)));

	garbage_pipeline(working_set, spans)
}

// Return false if command `export xxx` not found
// TODO: Rather than this, handle `export xxx` correctly in `parse_xxx`
fn warp_export_call(working_set: &mut StateWorkingSet, pipeline: &mut Pipeline, full_name: &str, spans: &[Span]) -> bool {
	let Some(export_decl_id) = working_set.find_decl(full_name.as_bytes()) else {
		let error_span = spans.first().cloned().unwrap_or(Span::unknown());
		working_set.error(ParseError::InternalError(format!("missing '{full_name}' command"), error_span));
		return false;
	};
	match pipeline.elements.first_mut().map(|e| {
		e.expr.span = Span::concat(spans);
		&mut e.expr.expr
	}) {
		Some(Expr::Call(def_call)) => {
			def_call.head = Span::concat(&spans[0..=1]);
			def_call.decl_id = export_decl_id;
			return true;
		}
		Some(Expr::AttributeBlock(ab)) => {
			if let Expr::Call(def_call) = &mut ab.item.expr {
				def_call.decl_id = export_decl_id;
				return true;
			}
		}
		_ => {}
	};
	working_set.error(ParseError::InternalError(
		"unexpected output from parsing a definition".into(),
		Span::concat(&spans[1..]),
	));
	true
}

// This one will trigger if `export` appears during eval, e.g., in a script
pub fn parse_export_in_block(working_set: &mut StateWorkingSet, lite_command: &LiteCommand) -> Pipeline {
	let parts = lite_command.command_parts();
	let full_name = if parts.len() > 1 {
		let sub = working_set.get_span_contents(parts[1]);
		match sub {
			b"alias" => "export alias",
			b"def" => "export def",
			b"extern" => "export extern",
			b"use" => "export use",
			b"module" => "export module",
			b"const" => "export const",
			_ => "export",
		}
	} else {
		"export"
	};

	if let Some(redirection) = lite_command.redirection.as_ref() {
		working_set.error(redirecting_builtin_error(full_name, redirection));
		return garbage_pipeline(working_set, &lite_command.parts);
	}

	let mut pipeline = match full_name {
		// `parse_def` and `parse_extern` work both with and without attributes
		"export def" => parse_def(working_set, lite_command, None).0,
		"export extern" => parse_extern(working_set, lite_command, None),
		// Other definitions can't have attributes, so we handle attributes here with parse_attribute_block
		_ if lite_command.has_attributes() => parse_attribute_block(working_set, lite_command),
		"export alias" => parse_alias(working_set, lite_command, None),
		"export const" => parse_const(working_set, &lite_command.parts[1..]).0,
		"export use" => parse_use(working_set, lite_command, None).0,
		"export module" => parse_module(working_set, lite_command, None).0,
		_ => {
			if let Some(decl_id) = working_set.find_decl(full_name.as_bytes()) {
				let starting_error_count = working_set.parse_errors.len();
				let ParsedInternalCall { call, output, call_kind } =
					parse_internal_call(working_set, parts[0], &parts[1..], decl_id, ArgumentParsingLevel::Full);

				if call_kind != CallKind::Valid {
					return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(&lite_command.parts), output)]);
				}
				// don't need errors generated by parse_internal_call
				working_set.parse_errors.truncate(starting_error_count);
				working_set.error(ParseError::UnexpectedKeyword(full_name.into(), lite_command.parts[0]));
			} else {
				working_set.error(ParseError::UnknownState(
					format!("internal error: '{full_name}' declaration not found",),
					Span::concat(&lite_command.parts),
				));
			};
			garbage_pipeline(working_set, &lite_command.parts)
		}
	};

	// HACK: This is for different messages of e.g. `export def --help` and `def --help`,
	warp_export_call(working_set, &mut pipeline, full_name, &lite_command.parts);
	pipeline
}

// This one will trigger only in a module
pub fn parse_export_in_module(
	working_set: &mut StateWorkingSet,
	lite_command: &LiteCommand,
	module_name: &[u8],
	parent_module: &mut Module,
) -> (Pipeline, Vec<Exportable>) {
	let spans = lite_command.command_parts();

	let export_span = if let Some(sp) = spans.first() {
		if working_set.get_span_contents(*sp) != b"export" {
			working_set.error(ParseError::UnknownState("expected export statement".into(), Span::concat(spans)));
			return (garbage_pipeline(working_set, spans), vec![]);
		}

		*sp
	} else {
		working_set.error(ParseError::UnknownState(
			"got empty input for parsing export statement".into(),
			Span::concat(spans),
		));
		return (garbage_pipeline(working_set, spans), vec![]);
	};

	let (pipeline, exportables) = if let Some(kw_span) = spans.get(1) {
		let kw_name = working_set.get_span_contents(*kw_span);
		match kw_name {
			// `parse_def` and `parse_extern` work both with and without attributes
			b"def" => {
				let (mut pipeline, cmd_result) = parse_def(working_set, lite_command, Some(module_name));

				let mut result = vec![];

				if let Some((decl_name, decl_id)) = cmd_result {
					result.push(Exportable::Decl {
						name: decl_name.to_vec(),
						id: decl_id,
					});
				}

				// Trying to warp the 'def' call into the 'export def' in a very clumsy way
				if !warp_export_call(working_set, &mut pipeline, "export def", spans) {
					return (garbage_pipeline(working_set, spans), vec![]);
				}

				(pipeline, result)
			}
			b"extern" => {
				let mut pipeline = parse_extern(working_set, lite_command, Some(module_name));

				// Trying to warp the 'extern' call into the 'export extern' in a very clumsy way
				if !warp_export_call(working_set, &mut pipeline, "export extern", spans) {
					return (garbage_pipeline(working_set, spans), vec![]);
				}

				let mut result = vec![];

				let decl_name = match spans.get(2) {
					Some(span) => working_set.get_span_contents(*span),
					None => &[],
				};
				let decl_name = trim_quotes(decl_name);

				if let Some(decl_id) = working_set.find_decl(decl_name) {
					result.push(Exportable::Decl {
						name: decl_name.to_vec(),
						id: decl_id,
					});
				} else {
					working_set.error(ParseError::InternalError("failed to find added declaration".into(), Span::concat(&spans[1..])));
				}

				(pipeline, result)
			}
			// Other definitions can't have attributes, so we handle attributes here with parse_attribute_block
			_ if lite_command.has_attributes() => (parse_attribute_block(working_set, lite_command), vec![]),
			b"alias" => {
				let lite_command = LiteCommand {
					comments: lite_command.comments.clone(),
					parts: spans[1..].to_vec(),
					pipe: lite_command.pipe,
					redirection: lite_command.redirection.clone(),
					attribute_idx: vec![],
				};
				let mut pipeline = parse_alias(working_set, &lite_command, Some(module_name));

				// Trying to warp the 'alias' call into the 'export alias' in a very clumsy way
				if !warp_export_call(working_set, &mut pipeline, "export alias", spans) {
					return (garbage_pipeline(working_set, spans), vec![]);
				}

				let mut result = vec![];

				let alias_name = match spans.get(2) {
					Some(span) => working_set.get_span_contents(*span),
					None => &[],
				};
				let alias_name = trim_quotes(alias_name);

				if let Some(alias_id) = working_set.find_decl(alias_name) {
					result.push(Exportable::Decl {
						name: alias_name.to_vec(),
						id: alias_id,
					});
				} else {
					working_set.error(ParseError::InternalError("failed to find added alias".into(), Span::concat(&spans[1..])));
				}

				(pipeline, result)
			}
			b"use" => {
				let lite_command = LiteCommand {
					comments: lite_command.comments.clone(),
					parts: spans[1..].to_vec(),
					pipe: lite_command.pipe,
					redirection: lite_command.redirection.clone(),
					attribute_idx: vec![],
				};
				let (mut pipeline, exportables) = parse_use(working_set, &lite_command, Some(parent_module));

				// Trying to warp the 'use' call into the 'export use' in a very clumsy way
				if !warp_export_call(working_set, &mut pipeline, "export use", spans) {
					return (garbage_pipeline(working_set, spans), vec![]);
				}

				(pipeline, exportables)
			}
			b"module" => {
				let (mut pipeline, maybe_module_id) = parse_module(working_set, lite_command, Some(module_name));

				// Trying to warp the 'module' call into the 'export module' in a very clumsy way
				if !warp_export_call(working_set, &mut pipeline, "export module", spans) {
					return (garbage_pipeline(working_set, spans), vec![]);
				}

				let mut result = vec![];

				if let Some(module_name_span) = spans.get(2) {
					let module_name = working_set.get_span_contents(*module_name_span);
					let module_name = trim_quotes(module_name);

					if let Some(module_id) = maybe_module_id {
						result.push(Exportable::Module {
							name: working_set.get_module(module_id).name(),
							id: module_id,
						});
					} else {
						working_set.error(ParseError::InternalError(
							format!("failed to find added module '{}'", String::from_utf8_lossy(module_name)),
							Span::concat(&spans[1..]),
						));
					}
				}

				(pipeline, result)
			}
			b"const" => {
				let (mut pipeline, var_name_span) = parse_const(working_set, &spans[1..]);

				// Trying to warp the 'const' call into the 'export const' in a very clumsy way
				if !warp_export_call(working_set, &mut pipeline, "export const", spans) {
					return (garbage_pipeline(working_set, spans), vec![]);
				}

				let mut result = vec![];

				if let Some(var_name_span) = var_name_span {
					let var_name = working_set.get_span_contents(var_name_span);
					let var_name = trim_quotes(var_name);

					if let Some(var_id) = working_set.find_variable(var_name) {
						if let Err(err) = working_set.get_constant(var_id) {
							working_set.error(err);
						} else {
							result.push(Exportable::VarDecl {
								name: var_name.to_vec(),
								id: var_id,
							});
						}
					} else {
						working_set.error(ParseError::InternalError("failed to find added variable".into(), Span::concat(&spans[1..])));
					}
				}

				(pipeline, result)
			}
			_ => {
				working_set.error(ParseError::Expected("def, alias, use, module, const or extern keyword", spans[1]));

				(garbage_pipeline(working_set, spans), vec![])
			}
		}
	} else {
		working_set.error(ParseError::MissingPositional(
			"def, alias, use, module, const or extern keyword".to_string(),
			Span::new(export_span.end, export_span.end),
			"def, alias, use, module, const or extern keyword".to_string(),
		));

		(garbage_pipeline(working_set, spans), vec![])
	};

	(pipeline, exportables)
}

pub fn parse_export_env(working_set: &mut StateWorkingSet, spans: &[Span]) -> (Pipeline, Option<BlockId>) {
	if !spans.is_empty() && working_set.get_span_contents(spans[0]) != b"export-env" {
		working_set.error(ParseError::UnknownState(
			"internal error: Wrong call name for 'export-env' command".into(),
			Span::concat(spans),
		));
		return (garbage_pipeline(working_set, spans), None);
	}

	if spans.len() < 2 {
		working_set.error(ParseError::MissingPositional("block".into(), Span::concat(spans), "export-env <block>".into()));
		return (garbage_pipeline(working_set, spans), None);
	}

	let call = match working_set.find_decl(b"export-env") {
		Some(decl_id) => {
			let ParsedInternalCall { call, output, call_kind } = parse_internal_call(working_set, spans[0], &[spans[1]], decl_id, ArgumentParsingLevel::Full);

			if call_kind != CallKind::Valid {
				return (
					Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), output)]),
					None,
				);
			}

			call
		}
		None => {
			working_set.error(ParseError::UnknownState(
				"internal error: 'export-env' declaration not found".into(),
				Span::concat(spans),
			));
			return (garbage_pipeline(working_set, spans), None);
		}
	};

	let block_id = if let Some(block) = call.positional_nth(0) {
		if let Some(block_id) = block.as_block() {
			block_id
		} else {
			working_set.error(ParseError::UnknownState("internal error: 'export-env' block is not a block".into(), block.span));
			return (garbage_pipeline(working_set, spans), None);
		}
	} else {
		working_set.error(ParseError::UnknownState(
			"internal error: 'export-env' block is missing".into(),
			Span::concat(spans),
		));
		return (garbage_pipeline(working_set, spans), None);
	};

	let pipeline = Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)]);

	// Since modules are parser constructs, `export-env` blocks don't have anything to drive their
	// compilation when they are inside modules
	//
	// When they appear not inside module definitions but inside runnable code (script, `def`
	// block, etc), their body is used more or less like a closure, which are also compiled eagerly
	//
	// This handles both of these cases
	compile_block_with_id(working_set, block_id);

	(pipeline, Some(block_id))
}

fn collect_first_comments(tokens: &[Token]) -> Vec<Span> {
	let mut comments = vec![];

	let mut tokens_iter = tokens.iter().peekable();
	while let Some(token) = tokens_iter.next() {
		match token.contents {
			TokenContents::Comment => {
				comments.push(token.span);
			}
			TokenContents::Eol => {
				if let Some(Token {
					contents: TokenContents::Eol, ..
				}) = tokens_iter.peek()
					&& !comments.is_empty()
				{
					break;
				}
			}
			_ => {
				comments.clear();
				break;
			}
		}
	}

	comments
}

pub fn parse_module_block(working_set: &mut StateWorkingSet, span: Span, module_name: &[u8]) -> (Block, Module, Vec<Span>) {
	working_set.enter_scope();

	let source = working_set.get_span_contents(span);

	let (output, err) = lex(source, span.start, &[], &[], false);
	if let Some(err) = err {
		working_set.error(err)
	}

	let module_comments = collect_first_comments(&output);

	let (output, err) = lite_parse(&output, working_set);
	if let Some(err) = err {
		working_set.error(err)
	}

	for pipeline in &output.block {
		if pipeline.commands.len() == 1 {
			parse_def_predecl(working_set, pipeline.commands[0].command_parts());
		}
	}

	let mut module = Module::from_span(module_name.to_vec(), span);

	let mut block = Block::new_with_capacity(output.block.len());
	block.span = Some(span);

	for pipeline in output.block.iter() {
		if pipeline.commands.len() == 1 {
			let command = &pipeline.commands[0];

			let name = command.command_parts().first().map(|s| working_set.get_span_contents(*s)).unwrap_or(b"");

			match name {
				// `parse_def` and `parse_extern` work both with and without attributes
				b"def" => {
					block.pipelines.push(
						parse_def(
							working_set,
							command,
							None, // using commands named as the module locally is OK
						)
						.0,
					)
				}
				b"extern" => block.pipelines.push(parse_extern(working_set, command, None)),
				// `parse_export_in_module` also handles attributes by itself
				b"export" => {
					let (pipe, exportables) = parse_export_in_module(working_set, command, module_name, &mut module);

					for exportable in exportables {
						match exportable {
							Exportable::Decl { name, id } => {
								if &name == b"main" {
									if module.main.is_some() {
										let err_span = if !pipe.elements.is_empty() {
											if let Expr::Call(call) = &pipe.elements[0].expr.expr {
												call.head
											} else {
												pipe.elements[0].expr.span
											}
										} else {
											span
										};
										working_set.error(ParseError::ModuleDoubleMain(String::from_utf8_lossy(module_name).to_string(), err_span));
									} else {
										module.main = Some(id);
									}
								} else {
									module.add_decl(name, id);
								}
							}
							Exportable::Module { name, id } => {
								if &name == b"mod" {
									let (submodule_main, submodule_decls, submodule_submodules) = {
										let submodule = working_set.get_module(id);
										(submodule.main, submodule.decls(), submodule.submodules())
									};

									// Add submodule's decls to the parent module
									for (decl_name, decl_id) in submodule_decls {
										module.add_decl(decl_name, decl_id);
									}

									// Add submodule's main command to the parent module
									if let Some(main_decl_id) = submodule_main {
										if module.main.is_some() {
											let err_span = if !pipe.elements.is_empty() {
												if let Expr::Call(call) = &pipe.elements[0].expr.expr {
													call.head
												} else {
													pipe.elements[0].expr.span
												}
											} else {
												span
											};
											working_set.error(ParseError::ModuleDoubleMain(String::from_utf8_lossy(module_name).to_string(), err_span));
										} else {
											module.main = Some(main_decl_id);
										}
									}

									// Add submodule's submodules to the parent module
									for (submodule_name, submodule_id) in submodule_submodules {
										module.add_submodule(submodule_name, submodule_id);
									}
								} else {
									module.add_submodule(name, id);
								}
							}
							Exportable::VarDecl { name, id } => {
								module.add_variable(name, id);
							}
						}
					}

					block.pipelines.push(pipe)
				}
				// Other definitions can't have attributes, so we handle attributes here with parse_attribute_block
				_ if command.has_attributes() => block.pipelines.push(parse_attribute_block(working_set, command)),
				b"const" => block.pipelines.push(parse_const(working_set, &command.parts).0),
				b"alias" => {
					block.pipelines.push(parse_alias(
						working_set,
						command,
						None, // using aliases named as the module locally is OK
					))
				}
				b"use" => {
					let (pipeline, _) = parse_use(working_set, command, Some(&mut module));

					block.pipelines.push(pipeline)
				}
				b"module" => {
					let (pipeline, _) = parse_module(
						working_set,
						command,
						None, // using modules named as the module locally is OK
					);

					block.pipelines.push(pipeline)
				}
				b"export-env" => {
					let (pipe, maybe_env_block) = parse_export_env(working_set, &command.parts);

					if let Some(block_id) = maybe_env_block {
						module.add_env_block(block_id);
					}

					block.pipelines.push(pipe)
				}
				_ => {
					working_set.error(ParseError::ExpectedKeyword(
						"def, const, extern, alias, use, module, export or export-env keyword".into(),
						command.parts[0],
					));

					block.pipelines.push(garbage_pipeline(working_set, &command.parts))
				}
			}
		} else {
			working_set.error(ParseError::Expected("not a pipeline", span));
			block.pipelines.push(garbage_pipeline(working_set, &[span]))
		}
	}

	working_set.exit_scope();

	(block, module, module_comments)
}

fn module_needs_reloading(working_set: &StateWorkingSet, module_id: ModuleId) -> bool {
	let module = working_set.get_module(module_id);

	fn submodule_need_reloading(working_set: &StateWorkingSet, submodule_id: ModuleId) -> bool {
		let submodule = working_set.get_module(submodule_id);
		let submodule_changed = if let Some((file_path, file_id)) = &submodule.file {
			let existing_contents = working_set.get_contents_of_file(*file_id);
			let file_contents = file_path.read(working_set);

			if let (Some(existing), Some(new)) = (existing_contents, file_contents) {
				existing != new
			} else {
				false
			}
		} else {
			false
		};

		if submodule_changed {
			true
		} else {
			module_needs_reloading(working_set, submodule_id)
		}
	}

	let export_submodule_changed = module
		.submodules
		.iter()
		.any(|(_, submodule_id)| submodule_need_reloading(working_set, *submodule_id));

	if export_submodule_changed {
		return true;
	}

	module
		.imported_modules
		.iter()
		.any(|submodule_id| submodule_need_reloading(working_set, *submodule_id))
}

/// Parse a module from a file.
///
/// The module name is inferred from the stem of the file, unless specified in `name_override`.
fn parse_module_file(working_set: &mut StateWorkingSet, path: ParserPath, path_span: Span, name_override: Option<String>) -> Option<ModuleId> {
	// Infer the module name from the stem of the file, unless overridden.
	let module_name = if let Some(name) = name_override {
		name
	} else if let Some(stem) = path.file_stem() {
		stem.to_string_lossy().to_string()
	} else {
		working_set.error(ParseError::ModuleNotFound(path_span, path.path().to_string_lossy().to_string()));
		return None;
	};

	// Read the content of the module.
	let contents = if let Some(contents) = path.read(working_set) {
		contents
	} else {
		working_set.error(ParseError::ModuleNotFound(path_span, path.path().to_string_lossy().to_string()));
		return None;
	};

	let file_id = working_set.add_file(path.path().to_string_lossy().to_string(), &contents);
	let new_span = working_set.get_span_for_file(file_id);

	// Check if we've parsed the module before.
	if let Some(module_id) = working_set.find_module_by_span(new_span)
		&& !module_needs_reloading(working_set, module_id)
	{
		return Some(module_id);
	}

	// Add the file to the stack of files being processed.
	if let Err(e) = working_set.files.push(path.clone().path_buf(), path_span) {
		working_set.error(e);
		return None;
	}

	// Parse the module
	let (block, mut module, module_comments) = parse_module_block(working_set, new_span, module_name.as_bytes());

	// Remove the file from the stack of files being processed.
	working_set.files.pop();

	let _ = working_set.add_block(Arc::new(block));
	module.file = Some((path, file_id));
	let module_id = working_set.add_module(&module_name, module, module_comments);

	Some(module_id)
}

pub fn parse_module_file_or_dir(working_set: &mut StateWorkingSet, path: &[u8], path_span: Span, name_override: Option<String>) -> Option<ModuleId> {
	let (module_path_str, err) = unescape_unquote_string(path, path_span);
	if let Some(err) = err {
		working_set.error(err);
		return None;
	}

	#[allow(deprecated)]
	let cwd = working_set.get_cwd();

	let module_path = if let Some(path) = find_in_dirs(&module_path_str, working_set, &cwd, Some(LIB_DIRS_VAR)) {
		path
	} else {
		working_set.error(ParseError::ModuleNotFound(path_span, module_path_str));
		return None;
	};

	if module_path.is_dir() {
		if module_path.read_dir().is_none() {
			working_set.error(ParseError::ModuleNotFound(path_span, module_path.path().to_string_lossy().to_string()));
			return None;
		};

		let module_name = if let Some(stem) = module_path.file_stem() {
			stem.to_string_lossy().to_string()
		} else {
			working_set.error(ParseError::ModuleNotFound(path_span, module_path.path().to_string_lossy().to_string()));
			return None;
		};

		let mod_nu_path = module_path.clone().join("mod.nu");

		if !(mod_nu_path.exists() && mod_nu_path.is_file()) {
			working_set.error(ParseError::ModuleMissingModNuFile(module_path.path().to_string_lossy().to_string(), path_span));
			return None;
		}

		if let Some(module_id) = parse_module_file(working_set, mod_nu_path, path_span, name_override.or(Some(module_name))) {
			let module = working_set.get_module(module_id).clone();

			let module_name = String::from_utf8_lossy(&module.name).to_string();

			let module_comments = if let Some(comments) = working_set.get_module_comments(module_id) {
				comments.to_vec()
			} else {
				vec![]
			};

			let new_module_id = working_set.add_module(&module_name, module, module_comments);

			Some(new_module_id)
		} else {
			None
		}
	} else if module_path.is_file() {
		parse_module_file(working_set, module_path, path_span, name_override)
	} else {
		working_set.error(ParseError::ModuleNotFound(path_span, module_path.path().to_string_lossy().to_string()));
		None
	}
}

pub fn parse_module(working_set: &mut StateWorkingSet, lite_command: &LiteCommand, module_name: Option<&[u8]>) -> (Pipeline, Option<ModuleId>) {
	// TODO: Currently, module is closing over its parent scope (i.e., defs in the parent scope are
	// visible and usable in this module's scope). We want to disable that for files.

	let spans = &lite_command.parts;

	if let Some(redirection) = lite_command.redirection.as_ref() {
		working_set.error(redirecting_builtin_error("module", redirection));
		return (garbage_pipeline(working_set, spans), None);
	}

	let mut module_comments = lite_command.comments.clone();

	let split_id = if spans.len() > 1 && working_set.get_span_contents(spans[0]) == b"export" {
		2
	} else {
		1
	};

	let (mut call, call_span) = match working_set.find_decl(b"module") {
		Some(decl_id) => {
			let (command_spans, rest_spans) = spans.split_at(split_id);

			let ParsedInternalCall { call, output, call_kind } = parse_internal_call(
				working_set,
				Span::concat(command_spans),
				rest_spans,
				decl_id,
				ArgumentParsingLevel::FirstK { k: 1 },
			);

			let call_span = Span::concat(spans);
			if call_kind != CallKind::Valid {
				return (
					Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, output)]),
					None,
				);
			}

			(call, call_span)
		}
		None => {
			working_set.error(ParseError::UnknownState(
				"internal error: 'module' or 'export module' declaration not found".into(),
				Span::concat(spans),
			));
			return (garbage_pipeline(working_set, spans), None);
		}
	};

	let (module_name_or_path, module_name_or_path_span) = if let Some(name) = call.positional_nth(0) {
		if let Some(s) = name.as_string() {
			if let Some(mod_name) = module_name
				&& s.as_bytes() == mod_name
			{
				working_set.error(ParseError::NamedAsModule("module".to_string(), s, "mod".to_string(), name.span));
				return (
					Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, Type::Any)]),
					None,
				);
			}
			(s, name.span)
		} else {
			working_set.error(ParseError::UnknownState("internal error: name not a string".into(), Span::concat(spans)));
			return (garbage_pipeline(working_set, spans), None);
		}
	} else {
		working_set.error(ParseError::UnknownState("internal error: missing positional".into(), Span::concat(spans)));
		return (garbage_pipeline(working_set, spans), None);
	};

	if spans.len() == split_id + 1 {
		let pipeline = Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, Type::Any)]);

		if let Some(module_id) = parse_module_file_or_dir(working_set, module_name_or_path.as_bytes(), module_name_or_path_span, None) {
			return (pipeline, Some(module_id));
		} else {
			working_set.error(ParseError::ModuleNotFound(module_name_or_path_span, module_name_or_path));
			return (pipeline, None);
		}
	}

	if spans.len() < split_id + 2 {
		working_set.error(ParseError::UnknownState(
			"Expected structure: module <name> or module <name> <block>".into(),
			Span::concat(spans),
		));

		return (garbage_pipeline(working_set, spans), None);
	}

	let module_name = module_name_or_path;

	let block_expr_span = spans[split_id + 1];
	let block_bytes = working_set.get_span_contents(block_expr_span);
	let mut start = block_expr_span.start;
	let mut end = block_expr_span.end;

	if block_bytes.starts_with(b"{") {
		start += 1;
	} else {
		working_set.error(ParseError::Expected("block", block_expr_span));
		return (garbage_pipeline(working_set, spans), None);
	}

	if block_bytes.ends_with(b"}") {
		end -= 1;
	} else {
		working_set.error(ParseError::Unclosed("}".into(), Span::new(end, end)));
	}

	let block_content_span = Span::new(start, end);

	let (block, module, inner_comments) = parse_module_block(working_set, block_content_span, module_name.as_bytes());

	let block_id = working_set.add_block(Arc::new(block));

	module_comments.extend(inner_comments);
	let module_id = working_set.add_module(&module_name, module, module_comments);

	let block_expr = Expression::new(working_set, Expr::Block(block_id), block_expr_span, Type::Block);

	// NOTE: Command `module`/`export module` has only 1 flag `-h/--help` which is handled outside of this function
	// if more flags added in the future, then this needs to be updated to something like `set_kth_positional`
	if !call.set_kth_argument(1, Argument::Positional(block_expr)) {
		working_set.error(ParseError::InternalError("Failed to set the block argument".into(), block_expr_span));
	}

	(
		Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)]),
		Some(module_id),
	)
}

pub fn parse_use(working_set: &mut StateWorkingSet, lite_command: &LiteCommand, parent_module: Option<&mut Module>) -> (Pipeline, Vec<Exportable>) {
	let spans = &lite_command.parts;

	let (name_span, split_id) = if spans.len() > 1 && working_set.get_span_contents(spans[0]) == b"export" {
		(spans[1], 2)
	} else {
		(spans[0], 1)
	};

	let use_call = working_set.get_span_contents(name_span).to_vec();
	if use_call != b"use" {
		working_set.error(ParseError::UnknownState(
			"internal error: Wrong call name for 'use' command".into(),
			Span::concat(spans),
		));
		return (garbage_pipeline(working_set, spans), vec![]);
	}

	if working_set.get_span_contents(name_span) != b"use" {
		working_set.error(ParseError::UnknownState(
			"internal error: Wrong call name for 'use' command".into(),
			Span::concat(spans),
		));
		return (garbage_pipeline(working_set, spans), vec![]);
	}

	if let Some(redirection) = lite_command.redirection.as_ref() {
		working_set.error(redirecting_builtin_error("use", redirection));
		return (garbage_pipeline(working_set, spans), vec![]);
	}

	let (call, call_span, args_spans) = match working_set.find_decl(b"use") {
		Some(decl_id) => {
			let (command_spans, rest_spans) = spans.split_at(split_id);

			let ParsedInternalCall { call, output, call_kind } =
				parse_internal_call(working_set, Span::concat(command_spans), rest_spans, decl_id, ArgumentParsingLevel::Full);

			let call_span = Span::concat(spans);
			if call_kind != CallKind::Valid {
				return (
					Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, output)]),
					vec![],
				);
			}

			(call, call_span, rest_spans)
		}
		None => {
			working_set.error(ParseError::UnknownState(
				"internal error: 'use' declaration not found".into(),
				Span::concat(spans),
			));
			return (garbage_pipeline(working_set, spans), vec![]);
		}
	};

	let import_pattern_expr = parse_import_pattern(working_set, call.positional_iter(), args_spans);

	let import_pattern = match &import_pattern_expr {
		Expression { expr: Expr::Nothing, .. } => {
			let mut call = call;
			call.set_parser_info("noop".to_string(), Expression::new_unknown(Expr::Nothing, Span::unknown(), Type::Nothing));
			return (
				Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)]),
				vec![],
			);
		}
		Expression {
			expr: Expr::ImportPattern(import_pattern),
			..
		} => import_pattern.clone(),
		_ => {
			working_set.error(ParseError::UnknownState(
				"internal error: Import pattern positional is not import pattern".into(),
				import_pattern_expr.span,
			));
			return (garbage_pipeline(working_set, spans), vec![]);
		}
	};

	let (mut import_pattern, module, module_id) = if let Some(module_id) = import_pattern.head.id {
		let module = working_set.get_module(module_id).clone();
		(
			ImportPattern {
				head: ImportPatternHead {
					name: module.name.clone(),
					id: Some(module_id),
					span: import_pattern.head.span,
				},
				members: import_pattern.members,
				hidden: HashSet::new(),
				constants: vec![],
			},
			module,
			module_id,
		)
	} else if let Some(module_id) = parse_module_file_or_dir(working_set, &import_pattern.head.name, import_pattern.head.span, None) {
		let module = working_set.get_module(module_id).clone();
		(
			ImportPattern {
				head: ImportPatternHead {
					name: module.name.clone(),
					id: Some(module_id),
					span: import_pattern.head.span,
				},
				members: import_pattern.members,
				hidden: HashSet::new(),
				constants: vec![],
			},
			module,
			module_id,
		)
	} else {
		working_set.error(ParseError::ModuleNotFound(
			import_pattern.head.span,
			String::from_utf8_lossy(&import_pattern.head.name).to_string(),
		));
		return (
			Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, Type::Any)]),
			vec![],
		);
	};

	let mut imported_modules = vec![];
	let (definitions, errors) = module.resolve_import_pattern(working_set, module_id, &import_pattern.members, None, name_span, &mut imported_modules);

	working_set.parse_errors.extend(errors);

	let mut constants = vec![];

	for (name, const_vid) in definitions.constants {
		constants.push((name, const_vid));
	}

	for (name, const_val) in definitions.constant_values {
		let const_var_id = working_set.add_variable(name.clone(), name_span, const_val.get_type(), false);
		working_set.set_variable_const_val(const_var_id, const_val);
		constants.push((name, const_var_id));
	}

	let exportables = definitions
		.decls
		.iter()
		.map(|(name, decl_id)| Exportable::Decl {
			name: name.clone(),
			id: *decl_id,
		})
		.chain(definitions.modules.iter().map(|(name, module_id)| Exportable::Module {
			name: name.clone(),
			id: *module_id,
		}))
		.chain(constants.iter().map(|(name, variable_id)| Exportable::VarDecl {
			name: name.clone(),
			id: *variable_id,
		}))
		.collect();

	import_pattern.constants = constants.iter().map(|(_, id)| *id).collect();

	if let Some(m) = parent_module {
		m.track_imported_modules(&imported_modules)
	}
	// Extend the current scope with the module's exportables
	working_set.use_decls(definitions.decls);
	working_set.use_modules(definitions.modules);
	working_set.use_variables(constants);

	// Create a new Use command call to pass the import pattern as parser info
	let import_pattern_expr = Expression::new(working_set, Expr::ImportPattern(Box::new(import_pattern)), Span::concat(args_spans), Type::Any);

	let mut call = call;
	call.set_parser_info("import_pattern".to_string(), import_pattern_expr);

	(
		Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)]),
		exportables,
	)
}

pub fn parse_hide(working_set: &mut StateWorkingSet, lite_command: &LiteCommand) -> Pipeline {
	let spans = &lite_command.parts;

	if working_set.get_span_contents(spans[0]) != b"hide" {
		working_set.error(ParseError::UnknownState(
			"internal error: Wrong call name for 'hide' command".into(),
			Span::concat(spans),
		));
		return garbage_pipeline(working_set, spans);
	}
	if let Some(redirection) = lite_command.redirection.as_ref() {
		working_set.error(redirecting_builtin_error("hide", redirection));
		return garbage_pipeline(working_set, spans);
	}

	let (call, args_spans) = match working_set.find_decl(b"hide") {
		Some(decl_id) => {
			let ParsedInternalCall { call, output, call_kind } = parse_internal_call(working_set, spans[0], &spans[1..], decl_id, ArgumentParsingLevel::Full);

			if call_kind != CallKind::Valid {
				return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), output)]);
			}

			(call, &spans[1..])
		}
		None => {
			working_set.error(ParseError::UnknownState(
				"internal error: 'hide' declaration not found".into(),
				Span::concat(spans),
			));
			return garbage_pipeline(working_set, spans);
		}
	};

	let import_pattern_expr = parse_import_pattern(working_set, call.positional_iter(), args_spans);

	let import_pattern = if let Expression {
		expr: Expr::ImportPattern(import_pattern),
		..
	} = &import_pattern_expr
	{
		import_pattern.clone()
	} else {
		working_set.error(ParseError::UnknownState(
			"internal error: Import pattern positional is not import pattern".into(),
			import_pattern_expr.span,
		));
		return garbage_pipeline(working_set, spans);
	};

	let bytes = working_set.get_span_contents(spans[0]);

	if bytes == b"hide" && spans.len() >= 2 {
		for span in spans[1..].iter() {
			parse_string(working_set, *span);
		}

		// module used only internally, not saved anywhere
		let (is_module, module) = if let Some(module_id) = working_set.find_module(&import_pattern.head.name) {
			(true, working_set.get_module(module_id).clone())
		} else if import_pattern.members.is_empty() {
			// The pattern head can be:
			if let Some(id) = working_set.find_decl(&import_pattern.head.name) {
				// a custom command,
				let mut module = Module::new(b"tmp".to_vec());
				module.add_decl(import_pattern.head.name.clone(), id);

				(false, module)
			} else {
				// , or it could be an env var (handled by the engine)
				(false, Module::new(b"tmp".to_vec()))
			}
		} else {
			working_set.error(ParseError::ModuleNotFound(
				spans[1],
				String::from_utf8_lossy(&import_pattern.head.name).to_string(),
			));
			return garbage_pipeline(working_set, spans);
		};

		// This kind of inverts the import pattern matching found in parse_use()
		let decls_to_hide = if import_pattern.members.is_empty() {
			if is_module {
				module.decl_names_with_head(&import_pattern.head.name)
			} else {
				module.decl_names()
			}
		} else {
			match &import_pattern.members[0] {
				ImportPatternMember::Glob { .. } => module.decl_names(),
				ImportPatternMember::Name { name, span } => {
					let mut decls = vec![];

					if name == b"main" {
						if module.main.is_some() {
							decls.push(import_pattern.head.name.clone());
						} else {
							working_set.error(ParseError::ExportNotFound(*span));
						}
					} else if let Some(item) = module.decl_name_with_head(name, &import_pattern.head.name) {
						decls.push(item);
					} else {
						working_set.error(ParseError::ExportNotFound(*span));
					}

					decls
				}
				ImportPatternMember::List { names } => {
					let mut decls = vec![];

					for (name, span) in names {
						if name == b"main" {
							if module.main.is_some() {
								decls.push(import_pattern.head.name.clone());
							} else {
								working_set.error(ParseError::ExportNotFound(*span));
								break;
							}
						} else if let Some(item) = module.decl_name_with_head(name, &import_pattern.head.name) {
							decls.push(item);
						} else {
							working_set.error(ParseError::ExportNotFound(*span));
							break;
						}
					}

					decls
				}
			}
		};

		let import_pattern = {
			let decls: HashSet<Vec<u8>> = decls_to_hide.iter().cloned().collect();

			import_pattern.with_hidden(decls)
		};

		// TODO: `use spam; use spam foo; hide foo` will hide both `foo` and `spam foo` since
		// they point to the same DeclId. Do we want to keep it that way?
		working_set.hide_decls(&decls_to_hide);

		// Create a new Use command call to pass the new import pattern
		let import_pattern_expr = Expression::new(working_set, Expr::ImportPattern(Box::new(import_pattern)), Span::concat(args_spans), Type::Any);

		let mut call = call;
		call.set_parser_info("import_pattern".to_string(), import_pattern_expr);

		Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), Span::concat(spans), Type::Any)])
	} else {
		working_set.error(ParseError::UnknownState("Expected structure: hide <name>".into(), Span::concat(spans)));
		garbage_pipeline(working_set, spans)
	}
}
