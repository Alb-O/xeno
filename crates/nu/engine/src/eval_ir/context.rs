pub fn eval_ir_block<D: DebugContext>(
	engine_state: &EngineState,
	stack: &mut Stack,
	block: &Block,
	input: PipelineData,
) -> Result<PipelineExecutionData, ShellError> {
	// Rust does not check recursion limits outside of const evaluation.
	// But nu programs run in the same process as the shell.
	// To prevent a stack overflow in user code from crashing the shell,
	// we limit the recursion depth of function calls.
	let maximum_call_stack_depth: u64 = engine_state.config.recursion_limit as u64;
	if stack.recursion_count > maximum_call_stack_depth {
		return Err(ShellError::RecursionLimitReached {
			recursion_limit: maximum_call_stack_depth,
			span: block.span,
		});
	}

	if let Some(ir_block) = &block.ir_block {
		D::enter_block(engine_state, block);

		let args_base = stack.arguments.get_base();
		let error_handler_base = stack.error_handlers.get_base();
		let finally_handler_base = stack.finally_run_handlers.get_base();

		// Allocate and initialize registers. I've found that it's not really worth trying to avoid
		// the heap allocation here by reusing buffers - our allocator is fast enough
		let mut registers = Vec::with_capacity(ir_block.register_count as usize);
		for _ in 0..ir_block.register_count {
			registers.push(PipelineExecutionData::empty());
		}

		// Initialize file storage.
		let mut files = vec![None; ir_block.file_count as usize];

		let result = eval_ir_block_impl::<D>(
			&mut EvalContext {
				engine_state,
				stack,
				data: &ir_block.data,
				block_span: &block.span,
				args_base,
				error_handler_base,
				finally_handler_base,
				redirect_out: None,
				redirect_err: None,
				matches: vec![],
				registers: &mut registers[..],
				files: &mut files[..],
			},
			ir_block,
			input,
		);

		stack.error_handlers.leave_frame(error_handler_base);
		stack.finally_run_handlers.leave_frame(finally_handler_base);
		stack.arguments.leave_frame(args_base);

		D::leave_block(engine_state, block);

		result
	} else {
		// FIXME blocks having IR should not be optional
		Err(ShellError::GenericError {
			error: "Can't evaluate block in IR mode".into(),
			msg: "block is missing compiled representation".into(),
			span: block.span,
			help: Some("the IrBlock is probably missing due to a compilation error".into()),
			inner: vec![],
		})
	}
}

/// All of the pointers necessary for evaluation
struct EvalContext<'a> {
	engine_state: &'a EngineState,
	stack: &'a mut Stack,
	data: &'a Arc<[u8]>,
	/// The span of the block
	block_span: &'a Option<Span>,
	/// Base index on the argument stack to reset to after a call
	args_base: usize,
	/// Base index on the error handler stack to reset to after a call
	error_handler_base: usize,
	/// Base index on the finally handler stack to reset to after a call
	finally_handler_base: usize,
	/// State set by redirect-out
	redirect_out: Option<Redirection>,
	/// State set by redirect-err
	redirect_err: Option<Redirection>,
	/// Scratch space to use for `match`
	matches: Vec<(VarId, Value)>,
	/// Intermediate pipeline data storage used by instructions, indexed by RegId
	registers: &'a mut [PipelineExecutionData],
	/// Holds open files used by redirections
	files: &'a mut [Option<Arc<File>>],
}

impl<'a> EvalContext<'a> {
	/// Replace the contents of a register with a new value
	#[inline]
	fn put_reg(&mut self, reg_id: RegId, new_value: PipelineExecutionData) {
		// log::trace!("{reg_id} <- {new_value:?}");
		self.registers[reg_id.get() as usize] = new_value;
	}

	/// Borrow the contents of a register.
	#[inline]
	fn borrow_reg(&self, reg_id: RegId) -> &PipelineData {
		&self.registers[reg_id.get() as usize]
	}

	/// Replace the contents of a register with `Empty` and then return the value that it contained
	#[inline]
	fn take_reg(&mut self, reg_id: RegId) -> PipelineExecutionData {
		// log::trace!("<- {reg_id}");
		std::mem::replace(&mut self.registers[reg_id.get() as usize], PipelineExecutionData::empty())
	}

	/// Clone data from a register. Must be collected first.
	fn clone_reg(&mut self, reg_id: RegId, error_span: Span) -> Result<PipelineData, ShellError> {
		// NOTE: here just clone the inner PipelineData
		// it's suitable for current usage.
		match &self.registers[reg_id.get() as usize].body {
			PipelineData::Empty => Ok(PipelineData::empty()),
			PipelineData::Value(val, meta) => Ok(PipelineData::value(val.clone(), meta.clone())),
			_ => Err(ShellError::IrEvalError {
				msg: "Must collect to value before using instruction that clones from a register".into(),
				span: Some(error_span),
			}),
		}
	}

	/// Clone a value from a register. Must be collected first.
	fn clone_reg_value(&mut self, reg_id: RegId, fallback_span: Span) -> Result<Value, ShellError> {
		match self.clone_reg(reg_id, fallback_span)? {
			PipelineData::Empty => Ok(Value::nothing(fallback_span)),
			PipelineData::Value(val, _) => Ok(val),
			_ => unreachable!("clone_reg should never return stream data"),
		}
	}

	/// Take and implicitly collect a register to a value
	fn collect_reg(&mut self, reg_id: RegId, fallback_span: Span) -> Result<Value, ShellError> {
		// NOTE: in collect, it maybe good to pick the inner PipelineData
		// directly, and drop the ExitStatus queue.
		let data = self.take_reg(reg_id);
		let data = data.body;
		let span = data.span().unwrap_or(fallback_span);
		data.into_value(span)
	}

	/// Get a string from data or produce evaluation error if it's invalid UTF-8
	fn get_str(&self, slice: DataSlice, error_span: Span) -> Result<&'a str, ShellError> {
		std::str::from_utf8(&self.data[slice]).map_err(|_| ShellError::IrEvalError {
			msg: format!("data slice does not refer to valid UTF-8: {slice:?}"),
			span: Some(error_span),
		})
	}
}
