/// Helper to collect values into [`PipelineData`], preserving original span and metadata
///
/// The metadata is removed if it is the file data source, as that's just meant to mark streams.
fn collect(data: PipelineData, fallback_span: Span) -> Result<PipelineData, ShellError> {
	let span = data.span().unwrap_or(fallback_span);
	let metadata = data.metadata().and_then(|m| m.for_collect());
	let value = data.into_value(span)?;
	Ok(PipelineData::value(value, metadata))
}

/// Helper for drain behavior.
fn drain(ctx: &mut EvalContext<'_>, data: PipelineExecutionData) -> Result<InstructionResult, ShellError> {
	use self::InstructionResult::*;

	match data.body {
		PipelineData::ByteStream(stream, ..) => {
			let span = stream.span();
			let callback_spans = stream.get_caller_spans().clone();
			if let Err(mut err) = stream.drain() {
				ctx.stack.set_last_error(&err);
				if callback_spans.is_empty() {
					return Err(err);
				} else {
					for s in callback_spans {
						err = ShellError::EvalBlockWithInput { span: s, sources: vec![err] }
					}
					return Err(err);
				}
			} else {
				ctx.stack.set_last_exit_code(0, span);
			}
		}
		PipelineData::ListStream(stream, ..) => {
			let callback_spans = stream.get_caller_spans().clone();
			if let Err(mut err) = stream.drain() {
				if callback_spans.is_empty() {
					return Err(err);
				} else {
					for s in callback_spans {
						err = ShellError::EvalBlockWithInput { span: s, sources: vec![err] }
					}
					return Err(err);
				}
			}
		}
		PipelineData::Value(..) | PipelineData::Empty => {}
	}

	Ok(Continue)
}

/// Helper for drainIfEnd behavior
fn drain_if_end(ctx: &mut EvalContext<'_>, data: PipelineExecutionData) -> Result<PipelineData, ShellError> {
	let stack = &mut ctx.stack.push_redirection(ctx.redirect_out.clone(), ctx.redirect_err.clone());
	let result = data.body.drain_to_out_dests(ctx.engine_state, stack)?;

	Ok(result)
}

enum RedirectionStream {
	Out,
	Err,
}

/// Open a file for redirection
fn open_file(ctx: &EvalContext<'_>, path: &Value, append: bool) -> Result<Arc<File>, ShellError> {
	let path_expanded = expand_path_with(path.as_str()?, ctx.engine_state.cwd(Some(ctx.stack))?, true);
	let mut options = File::options();
	if append {
		options.append(true);
	} else {
		options.write(true).truncate(true);
	}
	let file = options
		.create(true)
		.open(&path_expanded)
		.map_err(|err| IoError::new(err, path.span(), path_expanded))?;
	Ok(Arc::new(file))
}

/// Set up a [`Redirection`] from a [`RedirectMode`]
fn eval_redirection(ctx: &mut EvalContext<'_>, mode: &RedirectMode, span: Span, which: RedirectionStream) -> Result<Option<Redirection>, ShellError> {
	match mode {
		RedirectMode::Pipe => Ok(Some(Redirection::Pipe(OutDest::Pipe))),
		RedirectMode::PipeSeparate => Ok(Some(Redirection::Pipe(OutDest::PipeSeparate))),
		RedirectMode::Value => Ok(Some(Redirection::Pipe(OutDest::Value))),
		RedirectMode::Null => Ok(Some(Redirection::Pipe(OutDest::Null))),
		RedirectMode::Inherit => Ok(Some(Redirection::Pipe(OutDest::Inherit))),
		RedirectMode::Print => Ok(Some(Redirection::Pipe(OutDest::Print))),
		RedirectMode::File { file_num } => {
			let file = ctx.files.get(*file_num as usize).cloned().flatten().ok_or_else(|| ShellError::IrEvalError {
				msg: format!("Tried to redirect to file #{file_num}, but it is not open"),
				span: Some(span),
			})?;
			Ok(Some(Redirection::File(file)))
		}
		RedirectMode::Caller => Ok(match which {
			RedirectionStream::Out => ctx.stack.pipe_stdout().cloned().map(Redirection::Pipe),
			RedirectionStream::Err => ctx.stack.pipe_stderr().cloned().map(Redirection::Pipe),
		}),
	}
}

/// Do an `iterate` instruction. This can be called repeatedly to get more values from an iterable
fn eval_iterate(ctx: &mut EvalContext<'_>, dst: RegId, stream: RegId, end_index: usize) -> Result<InstructionResult, ShellError> {
	let mut data = ctx.take_reg(stream);
	if let PipelineData::ListStream(list_stream, _) = &mut data.body {
		// Modify the stream, taking one value off, and branching if it's empty
		if let Some(val) = list_stream.next_value() {
			ctx.put_reg(dst, PipelineExecutionData::from(val.into_pipeline_data()));
			ctx.put_reg(stream, data); // put the stream back so it can be iterated on again
			Ok(InstructionResult::Continue)
		} else {
			ctx.put_reg(dst, PipelineExecutionData::empty());
			Ok(InstructionResult::Branch(end_index))
		}
	} else {
		// Convert the PipelineData to an iterator, and wrap it in a ListStream so it can be
		// iterated on
		let metadata = data.metadata();
		let span = data.span().unwrap_or(Span::unknown());
		ctx.put_reg(
			stream,
			PipelineExecutionData::from(PipelineData::list_stream(
				ListStream::new(data.body.into_iter(), span, Signals::EMPTY),
				metadata,
			)),
		);
		eval_iterate(ctx, dst, stream, end_index)
	}
}

/// Redirect environment from the callee stack to the caller stack
fn redirect_env(engine_state: &EngineState, caller_stack: &mut Stack, callee_stack: &Stack) {
	// TODO: make this more efficient
	// Grab all environment variables from the callee
	let caller_env_vars = caller_stack.get_env_var_names(engine_state);

	// remove env vars that are present in the caller but not in the callee
	// (the callee hid them)
	for var in caller_env_vars.iter() {
		if !callee_stack.has_env_var(engine_state, var) {
			caller_stack.remove_env_var(engine_state, var);
		}
	}

	// add new env vars from callee to caller
	for (var, value) in callee_stack.get_stack_env_vars() {
		caller_stack.add_env_var(var, value);
	}

	// set config to callee config, to capture any updates to that
	caller_stack.config.clone_from(&callee_stack.config);
}
