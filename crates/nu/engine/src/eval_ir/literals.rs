/// Load a literal value into a register
fn load_literal(ctx: &mut EvalContext<'_>, dst: RegId, lit: &Literal, span: Span) -> Result<InstructionResult, ShellError> {
	// `Literal::Empty` represents "no pipeline input" and should produce
	// `PipelineData::Empty`. This is distinct from `Literal::Nothing` which
	// represents the `null` value and should produce `PipelineData::Value(Value::Nothing)`.
	// Some commands (like `metadata`) distinguish between these when deciding
	// whether positional args are allowed.
	if matches!(lit, Literal::Empty) {
		ctx.put_reg(dst, PipelineExecutionData::empty());
	} else {
		let value = literal_value(ctx, lit, span)?;
		ctx.put_reg(dst, PipelineExecutionData::from(PipelineData::value(value, None)));
	}
	Ok(InstructionResult::Continue)
}

fn literal_value(ctx: &mut EvalContext<'_>, lit: &Literal, span: Span) -> Result<Value, ShellError> {
	Ok(match lit {
		Literal::Bool(b) => Value::bool(*b, span),
		Literal::Int(i) => Value::int(*i, span),
		Literal::Float(f) => Value::float(*f, span),
		Literal::Filesize(q) => Value::filesize(*q, span),
		Literal::Duration(q) => Value::duration(*q, span),
		Literal::Binary(bin) => Value::binary(&ctx.data[*bin], span),
		Literal::Block(block_id) | Literal::RowCondition(block_id) | Literal::Closure(block_id) => {
			let block = ctx.engine_state.get_block(*block_id);
			let captures = block
				.captures
				.iter()
				.map(|(var_id, span)| get_var(ctx, *var_id, *span).map(|val| (*var_id, val)))
				.collect::<Result<Vec<_>, ShellError>>()?;
			Value::closure(Closure { block_id: *block_id, captures }, span)
		}
		Literal::Range { start, step, end, inclusion } => {
			let start = ctx.collect_reg(*start, span)?;
			let step = ctx.collect_reg(*step, span)?;
			let end = ctx.collect_reg(*end, span)?;
			let range = Range::new(start, step, end, *inclusion, span)?;
			Value::range(range, span)
		}
		Literal::List { capacity } => Value::list(Vec::with_capacity(*capacity), span),
		Literal::Record { capacity } => Value::record(Record::with_capacity(*capacity), span),
		Literal::Filepath { val: path, no_expand } => {
			let path = ctx.get_str(*path, span)?;
			if *no_expand {
				Value::string(path, span)
			} else {
				let path = expand_path(path, true);
				Value::string(path.to_string_lossy(), span)
			}
		}
		Literal::Directory { val: path, no_expand } => {
			let path = ctx.get_str(*path, span)?;
			if path == "-" {
				Value::string("-", span)
			} else if *no_expand {
				Value::string(path, span)
			} else {
				let path = expand_path(path, true);
				Value::string(path.to_string_lossy(), span)
			}
		}
		Literal::GlobPattern { val, no_expand } => Value::glob(ctx.get_str(*val, span)?, *no_expand, span),
		Literal::String(s) => Value::string(ctx.get_str(*s, span)?, span),
		Literal::RawString(s) => Value::string(ctx.get_str(*s, span)?, span),
		Literal::CellPath(path) => Value::cell_path(CellPath::clone(path), span),
		Literal::Date(dt) => Value::date(**dt, span),
		Literal::Nothing => Value::nothing(span),
		// Empty is handled specially in load_literal and should never reach here
		Literal::Empty => Value::nothing(span),
	})
}

fn binary_op(ctx: &mut EvalContext<'_>, lhs_dst: RegId, op: &Operator, rhs: RegId, span: Span) -> Result<InstructionResult, ShellError> {
	let lhs_val = ctx.collect_reg(lhs_dst, span)?;
	let rhs_val = ctx.collect_reg(rhs, span)?;

	// Handle binary op errors early
	if let Value::Error { error, .. } = lhs_val {
		return Err(*error);
	}
	if let Value::Error { error, .. } = rhs_val {
		return Err(*error);
	}

	// We only have access to one span here, but the generated code usually adds a `span`
	// instruction to set the output span to the right span.
	let op_span = span;

	let result = match op {
		Operator::Comparison(cmp) => match cmp {
			Comparison::Equal => lhs_val.eq(op_span, &rhs_val, span)?,
			Comparison::NotEqual => lhs_val.ne(op_span, &rhs_val, span)?,
			Comparison::LessThan => lhs_val.lt(op_span, &rhs_val, span)?,
			Comparison::GreaterThan => lhs_val.gt(op_span, &rhs_val, span)?,
			Comparison::LessThanOrEqual => lhs_val.lte(op_span, &rhs_val, span)?,
			Comparison::GreaterThanOrEqual => lhs_val.gte(op_span, &rhs_val, span)?,
			Comparison::RegexMatch => lhs_val.regex_match(ctx.engine_state, op_span, &rhs_val, false, span)?,
			Comparison::NotRegexMatch => lhs_val.regex_match(ctx.engine_state, op_span, &rhs_val, true, span)?,
			Comparison::In => lhs_val.r#in(op_span, &rhs_val, span)?,
			Comparison::NotIn => lhs_val.not_in(op_span, &rhs_val, span)?,
			Comparison::Has => lhs_val.has(op_span, &rhs_val, span)?,
			Comparison::NotHas => lhs_val.not_has(op_span, &rhs_val, span)?,
			Comparison::StartsWith => lhs_val.starts_with(op_span, &rhs_val, span)?,
			Comparison::NotStartsWith => lhs_val.not_starts_with(op_span, &rhs_val, span)?,
			Comparison::EndsWith => lhs_val.ends_with(op_span, &rhs_val, span)?,
			Comparison::NotEndsWith => lhs_val.not_ends_with(op_span, &rhs_val, span)?,
		},
		Operator::Math(mat) => match mat {
			Math::Add => lhs_val.add(op_span, &rhs_val, span)?,
			Math::Subtract => lhs_val.sub(op_span, &rhs_val, span)?,
			Math::Multiply => lhs_val.mul(op_span, &rhs_val, span)?,
			Math::Divide => lhs_val.div(op_span, &rhs_val, span)?,
			Math::FloorDivide => lhs_val.floor_div(op_span, &rhs_val, span)?,
			Math::Modulo => lhs_val.modulo(op_span, &rhs_val, span)?,
			Math::Pow => lhs_val.pow(op_span, &rhs_val, span)?,
			Math::Concatenate => lhs_val.concat(op_span, &rhs_val, span)?,
		},
		Operator::Boolean(bl) => match bl {
			Boolean::Or => lhs_val.or(op_span, &rhs_val, span)?,
			Boolean::Xor => lhs_val.xor(op_span, &rhs_val, span)?,
			Boolean::And => lhs_val.and(op_span, &rhs_val, span)?,
		},
		Operator::Bits(bit) => match bit {
			Bits::BitOr => lhs_val.bit_or(op_span, &rhs_val, span)?,
			Bits::BitXor => lhs_val.bit_xor(op_span, &rhs_val, span)?,
			Bits::BitAnd => lhs_val.bit_and(op_span, &rhs_val, span)?,
			Bits::ShiftLeft => lhs_val.bit_shl(op_span, &rhs_val, span)?,
			Bits::ShiftRight => lhs_val.bit_shr(op_span, &rhs_val, span)?,
		},
		Operator::Assignment(_asg) => {
			return Err(ShellError::IrEvalError {
				msg: "can't eval assignment with the `binary-op` instruction".into(),
				span: Some(span),
			});
		}
	};

	ctx.put_reg(lhs_dst, PipelineExecutionData::from(PipelineData::value(result, None)));

	Ok(InstructionResult::Continue)
}
