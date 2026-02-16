/// Eval an IR block on the provided slice of registers.
fn eval_ir_block_impl<D: DebugContext>(ctx: &mut EvalContext<'_>, ir_block: &IrBlock, input: PipelineData) -> Result<PipelineExecutionData, ShellError> {
	if !ctx.registers.is_empty() {
		ctx.registers[0] = PipelineExecutionData::from(input);
	}

	// Program counter, starts at zero.
	let mut pc = 0;
	let need_backtrace = ctx.engine_state.get_env_var("NU_BACKTRACE").is_some();
	let mut ret_val = None;

	while pc < ir_block.instructions.len() {
		let instruction = &ir_block.instructions[pc];
		let span = &ir_block.spans[pc];
		let ast = &ir_block.ast[pc];

		D::enter_instruction(ctx.engine_state, ir_block, pc, ctx.registers);

		let result = eval_instruction::<D>(ctx, instruction, span, ast, need_backtrace);

		D::leave_instruction(ctx.engine_state, ir_block, pc, ctx.registers, result.as_ref().err());

		match result {
			Ok(InstructionResult::Continue) => {
				pc += 1;
			}
			Ok(InstructionResult::Branch(next_pc)) => {
				pc = next_pc;
			}
			Ok(InstructionResult::Return(reg_id)) => {
				// need to check if the return value is set by
				// `Shell::Return` first. If so, we need to respect that value.
				match ret_val {
					Some(err) => return Err(err),
					None => return Ok(ctx.take_reg(reg_id)),
				}
			}
			Err(err @ (ShellError::Continue { .. } | ShellError::Break { .. })) => {
				return Err(err);
			}
			Err(err @ (ShellError::Return { .. } | ShellError::Exit { .. })) => {
				if let Some(always_run_handler) = ctx.stack.finally_run_handlers.pop(ctx.finally_handler_base) {
					// need to run finally block before return.
					// and record the return value firstly.
					prepare_error_handler(ctx, always_run_handler, None);
					pc = always_run_handler.handler_index;
					ret_val = Some(err);
				} else {
					// These block control related errors should be passed through
					return Err(err);
				}
			}
			Err(err) => {
				if let Some(error_handler) = ctx.stack.error_handlers.pop(ctx.error_handler_base) {
					// If an error handler is set, branch there
					prepare_error_handler(ctx, error_handler, Some(err.into_spanned(*span)));
					pc = error_handler.handler_index;
				} else if let Some(always_run_handler) = ctx.stack.finally_run_handlers.pop(ctx.finally_handler_base) {
					prepare_error_handler(ctx, always_run_handler, Some(err.into_spanned(*span)));
					pc = always_run_handler.handler_index;
				} else if need_backtrace {
					let err = ShellError::into_chained(err, *span);
					return Err(err);
				} else {
					return Err(err);
				}
			}
		}
	}

	// Fell out of the loop, without encountering a Return.
	Err(ShellError::IrEvalError {
		msg: format!("Program counter out of range (pc={pc}, len={len})", len = ir_block.instructions.len(),),
		span: *ctx.block_span,
	})
}

/// Prepare the context for an error handler
fn prepare_error_handler(ctx: &mut EvalContext<'_>, error_handler: ErrorHandler, error: Option<Spanned<ShellError>>) {
	if let Some(reg_id) = error_handler.error_register {
		if let Some(error) = error {
			// Stack state has to be updated for stuff like LAST_EXIT_CODE
			ctx.stack.set_last_error(&error.item);
			// Create the error value and put it in the register
			ctx.put_reg(
				reg_id,
				PipelineExecutionData::from(
					error
						.item
						.into_full_value(&StateWorkingSet::new(ctx.engine_state), ctx.stack, error.span)
						.into_pipeline_data(),
				),
			);
		} else {
			// Set the register to empty
			ctx.put_reg(reg_id, PipelineExecutionData::empty());
		}
	}
}

/// The result of performing an instruction. Describes what should happen next
#[derive(Debug)]
enum InstructionResult {
	Continue,
	Branch(usize),
	Return(RegId),
}

/// Perform an instruction
fn eval_instruction<D: DebugContext>(
	ctx: &mut EvalContext<'_>,
	instruction: &Instruction,
	span: &Span,
	ast: &Option<IrAstRef>,
	need_backtrace: bool,
) -> Result<InstructionResult, ShellError> {
	use self::InstructionResult::*;

	// Check for interrupt if necessary
	instruction.check_interrupt(ctx.engine_state, span)?;

	// See the docs for `Instruction` for more information on what these instructions are supposed
	// to do.
	match instruction {
		Instruction::Unreachable => Err(ShellError::IrEvalError {
			msg: "Reached unreachable code".into(),
			span: Some(*span),
		}),
		Instruction::LoadLiteral { dst, lit } => load_literal(ctx, *dst, lit, *span),
		Instruction::LoadValue { dst, val } => {
			ctx.put_reg(*dst, PipelineExecutionData::from(Value::clone(val).into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::Move { dst, src } => {
			let val = ctx.take_reg(*src);
			ctx.put_reg(*dst, val);
			Ok(Continue)
		}
		Instruction::Clone { dst, src } => {
			let data = ctx.clone_reg(*src, *span)?;
			ctx.put_reg(*dst, PipelineExecutionData::from(data));
			Ok(Continue)
		}
		Instruction::Collect { src_dst } => {
			let data = ctx.take_reg(*src_dst);
			// NOTE: is it ok to just using `data.inner`?
			let value = collect(data.body, *span)?;
			ctx.put_reg(*src_dst, PipelineExecutionData::from(value));
			Ok(Continue)
		}
		Instruction::Span { src_dst } => {
			let mut data = ctx.take_reg(*src_dst);
			data.body = data.body.with_span(*span);
			ctx.put_reg(*src_dst, data);
			Ok(Continue)
		}
		Instruction::Drop { src } => {
			ctx.take_reg(*src);
			Ok(Continue)
		}
		Instruction::Drain { src } => {
			let data = ctx.take_reg(*src);
			drain(ctx, data)
		}
		Instruction::DrainIfEnd { src } => {
			let data = ctx.take_reg(*src);
			let res = drain_if_end(ctx, data)?;
			ctx.put_reg(*src, PipelineExecutionData::from(res));
			Ok(Continue)
		}
		Instruction::LoadVariable { dst, var_id } => {
			let value = get_var(ctx, *var_id, *span)?;
			ctx.put_reg(*dst, PipelineExecutionData::from(value.into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::StoreVariable { var_id, src } => {
			let value = ctx.collect_reg(*src, *span)?;
			// Perform runtime type checking and conversion for variable assignment
			if xeno_nu_experimental::ENFORCE_RUNTIME_ANNOTATIONS.get() {
				let variable = ctx.engine_state.get_var(*var_id);
				let converted_value = check_assignment_type(value, &variable.ty)?;
				ctx.stack.add_var(*var_id, converted_value);
			} else {
				ctx.stack.add_var(*var_id, value);
			}
			Ok(Continue)
		}
		Instruction::DropVariable { var_id } => {
			ctx.stack.remove_var(*var_id);
			Ok(Continue)
		}
		Instruction::LoadEnv { dst, key } => {
			let key = ctx.get_str(*key, *span)?;
			if let Some(value) = get_env_var_case_insensitive(ctx, key) {
				let new_value = value.clone().into_pipeline_data();
				ctx.put_reg(*dst, PipelineExecutionData::from(new_value));
				Ok(Continue)
			} else {
				// FIXME: using the same span twice, shouldn't this really be
				// EnvVarNotFoundAtRuntime? There are tests that depend on CantFindColumn though...
				Err(ShellError::CantFindColumn {
					col_name: key.into(),
					span: Some(*span),
					src_span: *span,
				})
			}
		}
		Instruction::LoadEnvOpt { dst, key } => {
			let key = ctx.get_str(*key, *span)?;
			let value = get_env_var_case_insensitive(ctx, key).cloned().unwrap_or(Value::nothing(*span));
			ctx.put_reg(*dst, PipelineExecutionData::from(value.into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::StoreEnv { key, src } => {
			let key = ctx.get_str(*key, *span)?;
			let value = ctx.collect_reg(*src, *span)?;

			let key = get_env_var_name_case_insensitive(ctx, key);

			if !is_automatic_env_var(&key) {
				let is_config = key == "config";
				let update_conversions = key == ENV_CONVERSIONS;

				ctx.stack.add_env_var(key.into_owned(), value.clone());

				if is_config {
					ctx.stack.update_config(ctx.engine_state)?;
				}
				if update_conversions {
					convert_env_vars(ctx.stack, ctx.engine_state, &value)?;
				}
				Ok(Continue)
			} else {
				Err(ShellError::AutomaticEnvVarSetManually {
					envvar_name: key.into(),
					span: *span,
				})
			}
		}
		Instruction::PushPositional { src } => {
			let val = ctx.collect_reg(*src, *span)?.with_span(*span);
			ctx.stack.arguments.push(Argument::Positional {
				span: *span,
				val,
				ast: ast.clone().map(|ast_ref| ast_ref.0),
			});
			Ok(Continue)
		}
		Instruction::AppendRest { src } => {
			let vals = ctx.collect_reg(*src, *span)?.with_span(*span);
			ctx.stack.arguments.push(Argument::Spread {
				span: *span,
				vals,
				ast: ast.clone().map(|ast_ref| ast_ref.0),
			});
			Ok(Continue)
		}
		Instruction::PushFlag { name } => {
			let data = ctx.data.clone();
			ctx.stack.arguments.push(Argument::Flag {
				data,
				name: *name,
				short: DataSlice::empty(),
				span: *span,
			});
			Ok(Continue)
		}
		Instruction::PushShortFlag { short } => {
			let data = ctx.data.clone();
			ctx.stack.arguments.push(Argument::Flag {
				data,
				name: DataSlice::empty(),
				short: *short,
				span: *span,
			});
			Ok(Continue)
		}
		Instruction::PushNamed { name, src } => {
			let val = ctx.collect_reg(*src, *span)?.with_span(*span);
			let data = ctx.data.clone();
			ctx.stack.arguments.push(Argument::Named {
				data,
				name: *name,
				short: DataSlice::empty(),
				span: *span,
				val,
				ast: ast.clone().map(|ast_ref| ast_ref.0),
			});
			Ok(Continue)
		}
		Instruction::PushShortNamed { short, src } => {
			let val = ctx.collect_reg(*src, *span)?.with_span(*span);
			let data = ctx.data.clone();
			ctx.stack.arguments.push(Argument::Named {
				data,
				name: DataSlice::empty(),
				short: *short,
				span: *span,
				val,
				ast: ast.clone().map(|ast_ref| ast_ref.0),
			});
			Ok(Continue)
		}
		Instruction::PushParserInfo { name, info } => {
			let data = ctx.data.clone();
			ctx.stack.arguments.push(Argument::ParserInfo {
				data,
				name: *name,
				info: info.clone(),
			});
			Ok(Continue)
		}
		Instruction::RedirectOut { mode } => {
			ctx.redirect_out = eval_redirection(ctx, mode, *span, RedirectionStream::Out)?;
			Ok(Continue)
		}
		Instruction::RedirectErr { mode } => {
			ctx.redirect_err = eval_redirection(ctx, mode, *span, RedirectionStream::Err)?;
			Ok(Continue)
		}
		Instruction::CheckErrRedirected { src } => match ctx.borrow_reg(*src) {
			_ => Err(ShellError::GenericError {
				error: "Can't redirect stderr of internal command output".into(),
				msg: "piping stderr only works on external commands".into(),
				span: Some(*span),
				help: None,
				inner: vec![],
			}),
		},
		Instruction::OpenFile { file_num, path, append } => {
			let path = ctx.collect_reg(*path, *span)?;
			let file = open_file(ctx, &path, *append)?;
			ctx.files[*file_num as usize] = Some(file);
			Ok(Continue)
		}
		Instruction::WriteFile { file_num, src } => {
			let src = ctx.take_reg(*src);
			let file = ctx.files.get(*file_num as usize).cloned().flatten().ok_or_else(|| ShellError::IrEvalError {
				msg: format!("Tried to write to file #{file_num}, but it is not open"),
				span: Some(*span),
			})?;
			let is_external = if let PipelineData::ByteStream(stream, ..) = &src.body {
				stream.source().is_external()
			} else {
				false
			};
			if let Err(err) = src.body.write_to(file.as_ref()) {
				if is_external {
					ctx.stack.set_last_error(&err);
				}
				Err(err)?
			} else {
				Ok(Continue)
			}
		}
		Instruction::CloseFile { file_num } => {
			if ctx.files[*file_num as usize].take().is_some() {
				Ok(Continue)
			} else {
				Err(ShellError::IrEvalError {
					msg: format!("Tried to close file #{file_num}, but it is not open"),
					span: Some(*span),
				})
			}
		}
		Instruction::Call { decl_id, src_dst } => {
			let input = ctx.take_reg(*src_dst);
			// take out exit status future first.
			let input_data = input.body;
			let mut result = eval_call::<D>(ctx, *decl_id, *span, input_data)?;
			if need_backtrace {
				match &mut result {
					PipelineData::ByteStream(s, ..) => s.push_caller_span(*span),
					PipelineData::ListStream(s, ..) => s.push_caller_span(*span),
					_ => (),
				};
			}
			// After eval_call, attach result's exit_status_future
			// to `original_exit`, so all exit_status_future are tracked
			// in the new PipelineData, and wrap it into `PipelineExecutionData`
			ctx.put_reg(*src_dst, PipelineExecutionData { body: result });
			Ok(Continue)
		}
		Instruction::StringAppend { src_dst, val } => {
			let string_value = ctx.collect_reg(*src_dst, *span)?;
			let operand_value = ctx.collect_reg(*val, *span)?;
			let string_span = string_value.span();

			let mut string = string_value.into_string()?;
			let operand = if let Value::String { val, .. } = operand_value {
				// Small optimization, so we don't have to copy the string *again*
				val
			} else {
				operand_value.to_expanded_string(", ", &ctx.stack.get_config(ctx.engine_state))
			};
			string.push_str(&operand);

			let new_string_value = Value::string(string, string_span);
			ctx.put_reg(*src_dst, PipelineExecutionData::from(new_string_value.into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::GlobFrom { src_dst, no_expand } => {
			let string_value = ctx.collect_reg(*src_dst, *span)?;
			let glob_value = if let Value::Glob { .. } = string_value {
				// It already is a glob, so don't touch it.
				string_value
			} else {
				// Treat it as a string, then cast
				let string = string_value.into_string()?;
				Value::glob(string, *no_expand, *span)
			};
			ctx.put_reg(*src_dst, PipelineExecutionData::from(glob_value.into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::ListPush { src_dst, item } => {
			let list_value = ctx.collect_reg(*src_dst, *span)?;
			let item = ctx.collect_reg(*item, *span)?;
			let list_span = list_value.span();
			let mut list = list_value.into_list()?;
			list.push(item);
			ctx.put_reg(*src_dst, PipelineExecutionData::from(Value::list(list, list_span).into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::ListSpread { src_dst, items } => {
			let list_value = ctx.collect_reg(*src_dst, *span)?;
			let items = ctx.collect_reg(*items, *span)?;
			let list_span = list_value.span();
			let items_span = items.span();
			let items = match items {
				Value::List { vals, .. } => vals,
				Value::Nothing { .. } => Vec::new(),
				_ => return Err(ShellError::CannotSpreadAsList { span: items_span }),
			};
			let mut list = list_value.into_list()?;
			list.extend(items);
			ctx.put_reg(*src_dst, PipelineExecutionData::from(Value::list(list, list_span).into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::RecordInsert { src_dst, key, val } => {
			let record_value = ctx.collect_reg(*src_dst, *span)?;
			let key = ctx.collect_reg(*key, *span)?;
			let val = ctx.collect_reg(*val, *span)?;
			let record_span = record_value.span();
			let mut record = record_value.into_record()?;

			let key = key.coerce_into_string()?;
			if let Some(old_value) = record.insert(&key, val) {
				return Err(ShellError::ColumnDefinedTwice {
					col_name: key,
					second_use: *span,
					first_use: old_value.span(),
				});
			}

			ctx.put_reg(*src_dst, PipelineExecutionData::from(Value::record(record, record_span).into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::RecordSpread { src_dst, items } => {
			let record_value = ctx.collect_reg(*src_dst, *span)?;
			let items = ctx.collect_reg(*items, *span)?;
			let record_span = record_value.span();
			let items_span = items.span();
			let mut record = record_value.into_record()?;
			let items = match items {
				Value::Record { val, .. } => val.into_owned(),
				Value::Nothing { .. } => Record::new(),
				_ => return Err(ShellError::CannotSpreadAsRecord { span: items_span }),
			};
			// Not using .extend() here because it doesn't handle duplicates
			for (key, val) in items {
				if let Some(first_value) = record.insert(&key, val) {
					return Err(ShellError::ColumnDefinedTwice {
						col_name: key,
						second_use: *span,
						first_use: first_value.span(),
					});
				}
			}
			ctx.put_reg(*src_dst, PipelineExecutionData::from(Value::record(record, record_span).into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::Not { src_dst } => {
			let bool = ctx.collect_reg(*src_dst, *span)?;
			let negated = !bool.as_bool()?;
			ctx.put_reg(*src_dst, PipelineExecutionData::from(Value::bool(negated, bool.span()).into_pipeline_data()));
			Ok(Continue)
		}
		Instruction::BinaryOp { lhs_dst, op, rhs } => binary_op(ctx, *lhs_dst, op, *rhs, *span),
		Instruction::FollowCellPath { src_dst, path } => {
			let data = ctx.take_reg(*src_dst);
			let path = ctx.take_reg(*path);
			if let PipelineData::Value(Value::CellPath { val: path, .. }, _) = path.body {
				let value = data.body.follow_cell_path(&path.members, *span)?;
				ctx.put_reg(*src_dst, PipelineExecutionData::from(value.into_pipeline_data()));
				Ok(Continue)
			} else if let PipelineData::Value(Value::Error { error, .. }, _) = path.body {
				Err(*error)
			} else {
				Err(ShellError::TypeMismatch {
					err_message: "expected cell path".into(),
					span: path.span().unwrap_or(*span),
				})
			}
		}
		Instruction::CloneCellPath { dst, src, path } => {
			let value = ctx.clone_reg_value(*src, *span)?;
			let path = ctx.take_reg(*path);
			if let PipelineData::Value(Value::CellPath { val: path, .. }, _) = path.body {
				let value = value.follow_cell_path(&path.members)?;
				ctx.put_reg(*dst, PipelineExecutionData::from(value.into_owned().into_pipeline_data()));
				Ok(Continue)
			} else if let PipelineData::Value(Value::Error { error, .. }, _) = path.body {
				Err(*error)
			} else {
				Err(ShellError::TypeMismatch {
					err_message: "expected cell path".into(),
					span: path.span().unwrap_or(*span),
				})
			}
		}
		Instruction::UpsertCellPath { src_dst, path, new_value } => {
			let data = ctx.take_reg(*src_dst);
			let metadata = data.metadata();
			// Change the span because we're modifying it
			let mut value = data.body.into_value(*span)?;
			let path = ctx.take_reg(*path);
			let new_value = ctx.collect_reg(*new_value, *span)?;
			if let PipelineData::Value(Value::CellPath { val: path, .. }, _) = path.body {
				value.upsert_data_at_cell_path(&path.members, new_value)?;
				ctx.put_reg(*src_dst, PipelineExecutionData::from(value.into_pipeline_data_with_metadata(metadata)));
				Ok(Continue)
			} else if let PipelineData::Value(Value::Error { error, .. }, _) = path.body {
				Err(*error)
			} else {
				Err(ShellError::TypeMismatch {
					err_message: "expected cell path".into(),
					span: path.span().unwrap_or(*span),
				})
			}
		}
		Instruction::Jump { index } => Ok(Branch(*index)),
		Instruction::BranchIf { cond, index } => {
			let data = ctx.take_reg(*cond);
			let data_span = data.span();
			let val = match data.body {
				PipelineData::Value(Value::Bool { val, .. }, _) => val,
				PipelineData::Value(Value::Error { error, .. }, _) => {
					return Err(*error);
				}
				_ => {
					return Err(ShellError::TypeMismatch {
						err_message: "expected bool".into(),
						span: data_span.unwrap_or(*span),
					});
				}
			};
			if val { Ok(Branch(*index)) } else { Ok(Continue) }
		}
		Instruction::BranchIfEmpty { src, index } => {
			let is_empty = matches!(ctx.borrow_reg(*src), PipelineData::Empty | PipelineData::Value(Value::Nothing { .. }, _));

			if is_empty { Ok(Branch(*index)) } else { Ok(Continue) }
		}
		Instruction::Match { pattern, src, index } => {
			let value = ctx.clone_reg_value(*src, *span)?;
			ctx.matches.clear();
			if pattern.match_value(&value, &mut ctx.matches) {
				// Match succeeded: set variables and branch
				for (var_id, match_value) in ctx.matches.drain(..) {
					ctx.stack.add_var(var_id, match_value);
				}
				Ok(Branch(*index))
			} else {
				// Failed to match, put back original value
				ctx.matches.clear();
				Ok(Continue)
			}
		}
		Instruction::CheckMatchGuard { src } => {
			if matches!(ctx.borrow_reg(*src), PipelineData::Value(Value::Bool { .. }, _)) {
				Ok(Continue)
			} else {
				Err(ShellError::MatchGuardNotBool { span: *span })
			}
		}
		Instruction::Iterate { dst, stream, end_index } => eval_iterate(ctx, *dst, *stream, *end_index),
		Instruction::OnError { index } => {
			ctx.stack.error_handlers.push(ErrorHandler {
				handler_index: *index,
				error_register: None,
			});
			Ok(Continue)
		}
		Instruction::OnErrorInto { index, dst } => {
			ctx.stack.error_handlers.push(ErrorHandler {
				handler_index: *index,
				error_register: Some(*dst),
			});
			Ok(Continue)
		}
		Instruction::Finally { index } => {
			ctx.stack.finally_run_handlers.push(ErrorHandler {
				handler_index: *index,
				error_register: None,
			});
			Ok(Continue)
		}
		Instruction::FinallyInto { index, dst } => {
			ctx.stack.finally_run_handlers.push(ErrorHandler {
				handler_index: *index,
				error_register: Some(*dst),
			});
			Ok(Continue)
		}
		Instruction::PopErrorHandler => {
			ctx.stack.error_handlers.pop(ctx.error_handler_base);
			Ok(Continue)
		}
		Instruction::PopFinallyRun => {
			ctx.stack.finally_run_handlers.pop(ctx.finally_handler_base);
			Ok(Continue)
		}
		Instruction::ReturnEarly { src } => {
			let val = ctx.collect_reg(*src, *span)?;
			Err(ShellError::Return {
				span: *span,
				value: Box::new(val),
			})
		}
		Instruction::Return { src } => Ok(Return(*src)),
	}
}
