pub const LIB_DIRS_VAR: &str = "NU_LIB_DIRS";

use crate::known_external::KnownExternal;
use crate::lite_parser::{LiteCommand, lite_parse};
use crate::parser::{
	ParsedInternalCall, garbage, garbage_pipeline, parse, parse_call, parse_expression, parse_full_signature, parse_import_pattern, parse_internal_call,
	parse_string, parse_var_with_opt_type, trim_quotes,
};
use crate::{Token, TokenContents, is_math_expression_like, lex, unescape_unquote_string};

/// These parser keywords can be aliased
pub const ALIASABLE_PARSER_KEYWORDS: &[&[u8]] = &[b"if", b"match", b"try", b"overlay", b"overlay hide", b"overlay new", b"overlay use"];

pub const RESERVED_VARIABLE_NAMES: [&str; 4] = ["in", "nu", "env", "it"];

pub fn ensure_not_reserved_variable_name(working_set: &mut StateWorkingSet, lvalue: &Expression) {
	if lvalue.as_var().is_none() {
		return;
	}

	let var_name = String::from_utf8_lossy(working_set.get_span_contents(lvalue.span))
		.trim_start_matches('$')
		.to_string();

	verify_not_reserved_variable_name(working_set, &var_name, lvalue.span);
}

/// These parser keywords cannot be aliased (either not possible, or support not yet added)
pub const UNALIASABLE_PARSER_KEYWORDS: &[&[u8]] = &[
	b"alias",
	b"const",
	b"def",
	b"extern",
	b"module",
	b"use",
	b"export",
	b"export alias",
	b"export const",
	b"export def",
	b"export extern",
	b"export module",
	b"export use",
	b"for",
	b"loop",
	b"while",
	b"return",
	b"break",
	b"continue",
	b"let",
	b"mut",
	b"hide",
	b"export-env",
	b"source-env",
	b"source",
	b"where",
	b"plugin use",
];

/// Check whether spans start with a parser keyword that can be aliased
pub fn is_unaliasable_parser_keyword(working_set: &StateWorkingSet, spans: &[Span]) -> bool {
	// try two words
	if let (Some(&span1), Some(&span2)) = (spans.first(), spans.get(1)) {
		let cmd_name = working_set.get_span_contents(Span::append(span1, span2));
		return UNALIASABLE_PARSER_KEYWORDS.contains(&cmd_name);
	}

	// try one word
	if let Some(&span1) = spans.first() {
		let cmd_name = working_set.get_span_contents(span1);
		UNALIASABLE_PARSER_KEYWORDS.contains(&cmd_name)
	} else {
		false
	}
}

/// This is a new more compact method of calling parse_xxx() functions without repeating the
/// parse_call() in each function. Remaining keywords can be moved here.
pub fn parse_keyword(working_set: &mut StateWorkingSet, lite_command: &LiteCommand) -> Pipeline {
	let orig_parse_errors_len = working_set.parse_errors.len();

	let call_expr = parse_call(working_set, &lite_command.parts, lite_command.parts[0]);

	// If an error occurred, don't invoke the keyword-specific functionality
	if working_set.parse_errors.len() > orig_parse_errors_len {
		return Pipeline::from_vec(vec![call_expr]);
	}

	if let Expression { expr: Expr::Call(call), .. } = call_expr.clone() {
		// Apply parse keyword side effects
		let cmd = working_set.get_decl(call.decl_id);
		// check help flag first.
		if call.named_iter().any(|(flag, _, _)| flag.item == "help") {
			let call_span = call.span();
			return Pipeline::from_vec(vec![Expression::new(working_set, Expr::Call(call), call_span, Type::Any)]);
		}

		match cmd.name() {
			"overlay hide" => parse_overlay_hide(working_set, call),
			"overlay new" => parse_overlay_new(working_set, call),
			"overlay use" => parse_overlay_use(working_set, call),
			_ => Pipeline::from_vec(vec![call_expr]),
		}
	} else {
		Pipeline::from_vec(vec![call_expr])
	}
}

pub fn parse_def_predecl(working_set: &mut StateWorkingSet, spans: &[Span]) {
	let mut pos = 0;

	let def_type_name = if spans.len() >= 3 {
		// definition can't have only two spans, minimum is 3, e.g., 'extern spam []'
		let first_word = working_set.get_span_contents(spans[0]);

		if first_word == b"export" {
			pos += 2;
		} else {
			pos += 1;
		}

		working_set.get_span_contents(spans[pos - 1]).to_vec()
	} else {
		return;
	};

	if def_type_name != b"def" && def_type_name != b"extern" {
		return;
	}

	// Now, pos should point at the next span after the def-like call.
	// Skip all potential flags, like --env, --wrapped or --help:
	while pos < spans.len() && working_set.get_span_contents(spans[pos]).starts_with(b"-") {
		pos += 1;
	}

	if pos >= spans.len() {
		// This can happen if the call ends with a flag, e.g., 'def --help'
		return;
	}

	// Now, pos should point at the command name.
	let name_pos = pos;

	let Some(name) = parse_string(working_set, spans[name_pos]).as_string() else {
		return;
	};

	if name.contains('#') || name.contains('^') || name.parse::<bytesize::ByteSize>().is_ok() || name.parse::<f64>().is_ok() {
		working_set.error(ParseError::CommandDefNotValid(spans[name_pos]));
		return;
	}

	// Find signature
	let mut signature_pos = None;

	while pos < spans.len() {
		if working_set.get_span_contents(spans[pos]).starts_with(b"[") || working_set.get_span_contents(spans[pos]).starts_with(b"(") {
			signature_pos = Some(pos);
			break;
		}

		pos += 1;
	}

	let Some(signature_pos) = signature_pos else {
		return;
	};

	let mut allow_unknown_args = false;

	for span in spans {
		if working_set.get_span_contents(*span) == b"--wrapped" && def_type_name == b"def" {
			allow_unknown_args = true;
		}
	}

	let starting_error_count = working_set.parse_errors.len();

	working_set.enter_scope();
	// FIXME: because parse_signature will update the scope with the variables it sees
	// we end up parsing the signature twice per def. The first time is during the predecl
	// so that we can see the types that are part of the signature, which we need for parsing.
	// The second time is when we actually parse the body itworking_set.
	// We can't reuse the first time because the variables that are created during parse_signature
	// are lost when we exit the scope below.
	let sig = parse_full_signature(working_set, &spans[signature_pos..]);
	working_set.parse_errors.truncate(starting_error_count);
	working_set.exit_scope();

	let Some(mut signature) = sig.as_signature() else {
		return;
	};

	signature.name = name;

	if allow_unknown_args {
		signature.allows_unknown_args = true;
	}

	let decl = signature.predeclare();

	if working_set.add_predecl(decl).is_some() {
		working_set.error(ParseError::DuplicateCommandDef(spans[name_pos]));
	}
}

pub fn parse_for(working_set: &mut StateWorkingSet, lite_command: &LiteCommand) -> Expression {
	let spans = &lite_command.parts;
	// Checking that the function is used with the correct name
	// Maybe this is not necessary but it is a sanity check
	if working_set.get_span_contents(spans[0]) != b"for" {
		working_set.error(ParseError::UnknownState(
			"internal error: Wrong call name for 'for' function".into(),
			Span::concat(spans),
		));
		return garbage(working_set, spans[0]);
	}
	if let Some(redirection) = lite_command.redirection.as_ref() {
		working_set.error(redirecting_builtin_error("for", redirection));
		return garbage(working_set, spans[0]);
	}

	// Parsing the spans and checking that they match the register signature
	// Using a parsed call makes more sense than checking for how many spans are in the call
	// Also, by creating a call, it can be checked if it matches the declaration signature
	let (call, call_span) = match working_set.find_decl(b"for") {
		None => {
			working_set.error(ParseError::UnknownState(
				"internal error: for declaration not found".into(),
				Span::concat(spans),
			));
			return garbage(working_set, spans[0]);
		}
		Some(decl_id) => {
			let starting_error_count = working_set.parse_errors.len();
			working_set.enter_scope();
			let ParsedInternalCall { call, output, call_kind } = parse_internal_call(working_set, spans[0], &spans[1..], decl_id, ArgumentParsingLevel::Full);

			if working_set
				.parse_errors
				.get(starting_error_count..)
				.is_none_or(|new_errors| new_errors.iter().all(|e| !matches!(e, ParseError::Unclosed(token, _) if token == "}")))
			{
				working_set.exit_scope();
			}

			let call_span = Span::concat(spans);
			let decl = working_set.get_decl(decl_id);
			let sig = decl.signature();

			if call_kind != CallKind::Valid {
				return Expression::new(working_set, Expr::Call(call), call_span, output);
			}

			// Let's get our block and make sure it has the right signature
			if let Some(
				Expression {
					expr: Expr::Block(block_id), ..
				}
				| Expression {
					expr: Expr::RowCondition(block_id),
					..
				},
			) = call.positional_nth(2)
			{
				{
					let block = working_set.get_block_mut(*block_id);

					block.signature = Box::new(sig);
				}
			}

			(call, call_span)
		}
	};

	// All positional arguments must be in the call positional vector by this point
	let var_decl = call.positional_nth(0).expect("for call already checked");
	let iteration_expr = call.positional_nth(1).expect("for call already checked");
	let block = call.positional_nth(2).expect("for call already checked");

	let iteration_expr_ty = iteration_expr.ty.clone();

	// Figure out the type of the variable the `for` uses for iteration
	let var_type = match iteration_expr_ty {
		Type::List(x) => *x,
		Type::Table(x) => Type::Record(x),
		Type::Range => Type::Number, // Range elements can be int or float
		x => x,
	};

	if let (Some(var_id), Some(block_id)) = (&var_decl.as_var(), block.as_block()) {
		working_set.set_variable_type(*var_id, var_type.clone());

		let block = working_set.get_block_mut(block_id);

		block.signature.required_positional.insert(
			0,
			PositionalArg {
				name: String::new(),
				desc: String::new(),
				shape: var_type.to_shape(),
				var_id: Some(*var_id),
				default_value: None,
				completion: None,
			},
		);
	}

	Expression::new(working_set, Expr::Call(call), call_span, Type::Nothing)
}

/// If `name` is a keyword, emit an error.
fn verify_not_reserved_variable_name(working_set: &mut StateWorkingSet, name: &str, span: Span) {
	if RESERVED_VARIABLE_NAMES.contains(&name) {
		working_set.error(ParseError::NameIsBuiltinVar(name.to_string(), span))
	}
}

// This is meant for parsing attribute blocks without an accompanying `def` or `extern`. It's
// necessary to provide consistent syntax highlighting, completions, and helpful errors
//
// There is no need to run the const evaluation here
pub fn parse_attribute_block(working_set: &mut StateWorkingSet, lite_command: &LiteCommand) -> Pipeline {
	let attributes = lite_command
		.attribute_commands()
		.map(|cmd| parse_attribute(working_set, &cmd).0)
		.collect::<Vec<_>>();

	let last_attr_span = attributes.last().expect("Attribute block must contain at least one attribute").expr.span;

	working_set.error(ParseError::AttributeRequiresDefinition(last_attr_span));
	let cmd_span = if lite_command.command_parts().is_empty() {
		last_attr_span.past()
	} else {
		Span::concat(lite_command.command_parts())
	};
	let cmd_expr = garbage(working_set, cmd_span);
	let ty = cmd_expr.ty.clone();

	let attr_block_span = Span::merge_many(attributes.first().map(|x| x.expr.span).into_iter().chain(Some(cmd_span)));

	Pipeline::from_vec(vec![Expression::new(
		working_set,
		Expr::AttributeBlock(AttributeBlock {
			attributes,
			item: Box::new(cmd_expr),
		}),
		attr_block_span,
		ty,
	)])
}

// Returns also the parsed command name and ID
pub fn parse_def(working_set: &mut StateWorkingSet, lite_command: &LiteCommand, module_name: Option<&[u8]>) -> (Pipeline, Option<(Vec<u8>, DeclId)>) {
	let mut attributes = vec![];
	let mut attribute_vals = vec![];

	for attr_cmd in lite_command.attribute_commands() {
		let (attr, name) = parse_attribute(working_set, &attr_cmd);
		if let Some(name) = name {
			let val = eval_constant(working_set, &attr.expr);
			match val {
				Ok(val) => attribute_vals.push((name, val)),
				Err(e) => working_set.error(e.wrap(working_set, attr.expr.span)),
			}
		}
		attributes.push(attr);
	}

	let (expr, decl) = parse_def_inner(working_set, attribute_vals, lite_command, module_name);

	let ty = expr.ty.clone();

	let attr_block_span = Span::merge_many(attributes.first().map(|x| x.expr.span).into_iter().chain(Some(expr.span)));

	let expr = if attributes.is_empty() {
		expr
	} else {
		Expression::new(
			working_set,
			Expr::AttributeBlock(AttributeBlock {
				attributes,
				item: Box::new(expr),
			}),
			attr_block_span,
			ty,
		)
	};

	(Pipeline::from_vec(vec![expr]), decl)
}

pub fn parse_extern(working_set: &mut StateWorkingSet, lite_command: &LiteCommand, module_name: Option<&[u8]>) -> Pipeline {
	let mut attributes = vec![];
	let mut attribute_vals = vec![];

	for attr_cmd in lite_command.attribute_commands() {
		let (attr, name) = parse_attribute(working_set, &attr_cmd);
		if let Some(name) = name {
			let val = eval_constant(working_set, &attr.expr);
			match val {
				Ok(val) => attribute_vals.push((name, val)),
				Err(e) => working_set.error(e.wrap(working_set, attr.expr.span)),
			}
		}
		attributes.push(attr);
	}

	let expr = parse_extern_inner(working_set, attribute_vals, lite_command, module_name);

	let ty = expr.ty.clone();

	let attr_block_span = Span::merge_many(attributes.first().map(|x| x.expr.span).into_iter().chain(Some(expr.span)));

	let expr = if attributes.is_empty() {
		expr
	} else {
		Expression::new(
			working_set,
			Expr::AttributeBlock(AttributeBlock {
				attributes,
				item: Box::new(expr),
			}),
			attr_block_span,
			ty,
		)
	};

	Pipeline::from_vec(vec![expr])
}

// Returns also the parsed command name and ID
fn parse_def_inner(
	working_set: &mut StateWorkingSet,
	attributes: Vec<(String, Value)>,
	lite_command: &LiteCommand,
	module_name: Option<&[u8]>,
) -> (Expression, Option<(Vec<u8>, DeclId)>) {
	let spans = lite_command.command_parts();

	let (desc, extra_desc) = working_set.build_desc(&lite_command.comments);
	let garbage_result = |working_set: &mut StateWorkingSet<'_>| (garbage(working_set, Span::concat(spans)), None);

	// Checking that the function is used with the correct name
	// Maybe this is not necessary but it is a sanity check
	// Note: "export def" is treated the same as "def"

	let (name_span, split_id) = if spans.len() > 1 && working_set.get_span_contents(spans[0]) == b"export" {
		(spans[1], 2)
	} else {
		(spans[0], 1)
	};

	let def_call = working_set.get_span_contents(name_span);
	if def_call != b"def" {
		working_set.error(ParseError::UnknownState(
			"internal error: Wrong call name for def function".into(),
			Span::concat(spans),
		));
		return garbage_result(working_set);
	}
	if let Some(redirection) = lite_command.redirection.as_ref() {
		working_set.error(redirecting_builtin_error("def", redirection));
		return garbage_result(working_set);
	}

	// Parsing the spans and checking that they match the register signature
	// Using a parsed call makes more sense than checking for how many spans are in the call
	// Also, by creating a call, it can be checked if it matches the declaration signature
	//
	// NOTE: Here we only search for `def` in the permanent state,
	// since recursively redefining `def` is dangerous,
	// see https://github.com/nushell/nushell/issues/16586
	let (call, call_span) = match working_set.permanent_state.find_decl(def_call, &[]) {
		None => {
			working_set.error(ParseError::UnknownState(
				"internal error: def declaration not found".into(),
				Span::concat(spans),
			));
			return garbage_result(working_set);
		}
		Some(decl_id) => {
			working_set.enter_scope();
			let (command_spans, rest_spans) = spans.split_at(split_id);

			// Find the first span that is not a flag
			let mut decl_name_span = None;

			for span in rest_spans {
				if !working_set.get_span_contents(*span).starts_with(b"-") {
					decl_name_span = Some(*span);
					break;
				}
			}

			if let Some(name_span) = decl_name_span {
				// Check whether name contains [] or () -- possible missing space error
				if let Some(err) = detect_params_in_name(working_set, name_span, decl_id) {
					working_set.error(err);
					return garbage_result(working_set);
				}
			}

			let starting_error_count = working_set.parse_errors.len();
			let ParsedInternalCall { call, output, call_kind } =
				parse_internal_call(working_set, Span::concat(command_spans), rest_spans, decl_id, ArgumentParsingLevel::Full);

			if working_set
				.parse_errors
				.get(starting_error_count..)
				.is_none_or(|new_errors| new_errors.iter().all(|e| !matches!(e, ParseError::Unclosed(token, _) if token == "}")))
			{
				working_set.exit_scope();
			}

			let call_span = Span::concat(spans);
			let decl = working_set.get_decl(decl_id);
			let sig = decl.signature();

			// Let's get our block and make sure it has the right signature
			if let Some(arg) = call.positional_nth(2) {
				match arg {
					Expression {
						expr: Expr::Closure(block_id), ..
					} => {
						// Custom command bodies' are compiled eagerly
						// 1.  `module`s are not compiled, since they aren't ran/don't have any
						//     executable code. So `def`s inside modules have to be compiled by
						//     themselves.
						// 2.  `def` calls in scripts/runnable code don't *run* any code either,
						//     they are handled completely by the parser.
						compile_block_with_id(working_set, *block_id);
						working_set.get_block_mut(*block_id).signature = Box::new(sig.clone());
					}
					_ => working_set.error(ParseError::Expected("definition body closure { ... }", arg.span)),
				}
			}

			if call_kind != CallKind::Valid {
				return (Expression::new(working_set, Expr::Call(call), call_span, output), None);
			}

			(call, call_span)
		}
	};

	let Ok(has_env) = has_flag_const(working_set, &call, "env") else {
		return garbage_result(working_set);
	};
	let Ok(has_wrapped) = has_flag_const(working_set, &call, "wrapped") else {
		return garbage_result(working_set);
	};

	// All positional arguments must be in the call positional vector by this point
	let name_expr = call.positional_nth(0).expect("def call already checked");
	let sig = call.positional_nth(1).expect("def call already checked");
	let block = call.positional_nth(2).expect("def call already checked");

	let name = if let Some(name) = name_expr.as_string() {
		if let Some(mod_name) = module_name
			&& name.as_bytes() == mod_name
		{
			let name_expr_span = name_expr.span;

			working_set.error(ParseError::NamedAsModule("command".to_string(), name, "main".to_string(), name_expr_span));
			return (Expression::new(working_set, Expr::Call(call), call_span, Type::Any), None);
		}

		name
	} else {
		working_set.error(ParseError::UnknownState("Could not get string from string expression".into(), name_expr.span));
		return garbage_result(working_set);
	};

	let mut result = None;

	if let (Some(mut signature), Some(block_id)) = (sig.as_signature(), block.as_block()) {
		for arg_name in &signature.required_positional {
			verify_not_reserved_variable_name(working_set, &arg_name.name, sig.span);
		}
		for arg_name in &signature.optional_positional {
			verify_not_reserved_variable_name(working_set, &arg_name.name, sig.span);
		}
		if let Some(arg_name) = &signature.rest_positional {
			verify_not_reserved_variable_name(working_set, &arg_name.name, sig.span);
		}
		for flag_name in &signature.get_names() {
			verify_not_reserved_variable_name(working_set, flag_name, sig.span);
		}

		if has_wrapped {
			if let Some(rest) = &signature.rest_positional {
				if let Some(var_id) = rest.var_id {
					let rest_var = &working_set.get_variable(var_id);

					if rest_var.ty != Type::Any && rest_var.ty != Type::List(Box::new(Type::String)) {
						working_set.error(ParseError::TypeMismatchHelp(
							Type::List(Box::new(Type::String)),
							rest_var.ty.clone(),
							rest_var.declaration_span,
							format!(
								"...rest-like positional argument used in 'def --wrapped' supports only strings. Change the type annotation of ...{} to 'string'.",
								&rest.name
							),
						));

						return (Expression::new(working_set, Expr::Call(call), call_span, Type::Any), result);
					}
				}
			} else {
				working_set.error(ParseError::MissingPositional(
					"...rest-like positional argument".to_string(),
					name_expr.span,
					"def --wrapped must have a ...rest-like positional argument. Add '...rest: string' to the command's signature.".to_string(),
				));

				return (Expression::new(working_set, Expr::Call(call), call_span, Type::Any), result);
			}
		}

		if let Some(decl_id) = working_set.find_predecl(name.as_bytes()) {
			signature.name.clone_from(&name);
			if !has_wrapped {
				*signature = signature.add_help();
			}
			signature.description = desc;
			signature.extra_description = extra_desc;
			signature.allows_unknown_args = has_wrapped;

			let (attribute_vals, examples) = handle_special_attributes(attributes, working_set, &mut signature);

			let declaration = working_set.get_decl_mut(decl_id);

			*declaration = signature.clone().into_block_command(block_id, attribute_vals, examples);

			let block = working_set.get_block_mut(block_id);
			block.signature = signature;
			block.redirect_env = has_env;

			if block.signature.input_output_types.is_empty() {
				block.signature.input_output_types.push((Type::Any, Type::Any));
			}

			let block = working_set.get_block(block_id);

			let typecheck_errors = check_block_input_output(working_set, block);

			working_set.parse_errors.extend_from_slice(&typecheck_errors);

			result = Some((name.as_bytes().to_vec(), decl_id));
		} else {
			working_set.error(ParseError::InternalError("Predeclaration failed to add declaration".into(), name_expr.span));
		};
	}

	// It's OK if it returns None: The decl was already merged in previous parse pass.
	working_set.merge_predecl(name.as_bytes());

	(Expression::new(working_set, Expr::Call(call), call_span, Type::Any), result)
}

fn parse_extern_inner(
	working_set: &mut StateWorkingSet,
	attributes: Vec<(String, Value)>,
	lite_command: &LiteCommand,
	module_name: Option<&[u8]>,
) -> Expression {
	let spans = lite_command.command_parts();

	let (description, extra_description) = working_set.build_desc(&lite_command.comments);

	// Checking that the function is used with the correct name
	// Maybe this is not necessary but it is a sanity check

	let (name_span, split_id) = if spans.len() > 1 && (working_set.get_span_contents(spans[0]) == b"export") {
		(spans[1], 2)
	} else {
		(spans[0], 1)
	};

	let extern_call = working_set.get_span_contents(name_span);
	if extern_call != b"extern" {
		working_set.error(ParseError::UnknownState(
			"internal error: Wrong call name for extern command".into(),
			Span::concat(spans),
		));
		return garbage(working_set, Span::concat(spans));
	}
	if let Some(redirection) = lite_command.redirection.as_ref() {
		working_set.error(redirecting_builtin_error("extern", redirection));
		return garbage(working_set, Span::concat(spans));
	}

	// Parsing the spans and checking that they match the register signature
	// Using a parsed call makes more sense than checking for how many spans are in the call
	// Also, by creating a call, it can be checked if it matches the declaration signature
	//
	// NOTE: Here we only search for `extern` in the permanent state,
	// since recursively redefining `extern` is dangerous,
	// see https://github.com/nushell/nushell/issues/16586
	let (call, call_span) = match working_set.permanent().find_decl(extern_call, &[]) {
		None => {
			working_set.error(ParseError::UnknownState(
				"internal error: def declaration not found".into(),
				Span::concat(spans),
			));
			return garbage(working_set, Span::concat(spans));
		}
		Some(decl_id) => {
			working_set.enter_scope();

			let (command_spans, rest_spans) = spans.split_at(split_id);

			if let Some(name_span) = rest_spans.first()
				&& let Some(err) = detect_params_in_name(working_set, *name_span, decl_id)
			{
				working_set.error(err);
				return garbage(working_set, Span::concat(spans));
			}

			let ParsedInternalCall { call, .. } =
				parse_internal_call(working_set, Span::concat(command_spans), rest_spans, decl_id, ArgumentParsingLevel::Full);
			working_set.exit_scope();

			let call_span = Span::concat(spans);

			(call, call_span)
		}
	};
	let name_expr = call.positional_nth(0);
	let sig = call.positional_nth(1);
	let body = call.positional_nth(2);

	if let (Some(name_expr), Some(sig)) = (name_expr, sig) {
		if let (Some(name), Some(mut signature)) = (&name_expr.as_string(), sig.as_signature()) {
			if let Some(mod_name) = module_name
				&& name.as_bytes() == mod_name
			{
				let name_expr_span = name_expr.span;
				working_set.error(ParseError::NamedAsModule(
					"known external".to_string(),
					name.clone(),
					"main".to_string(),
					name_expr_span,
				));
				return Expression::new(working_set, Expr::Call(call), call_span, Type::Any);
			}

			if let Some(decl_id) = working_set.find_predecl(name.as_bytes()) {
				let external_name = if let Some(mod_name) = module_name {
					if name.as_bytes() == b"main" {
						String::from_utf8_lossy(mod_name).to_string()
					} else {
						name.clone()
					}
				} else {
					name.clone()
				};

				signature.name = external_name;
				signature.description = description;
				signature.extra_description = extra_description;
				signature.allows_unknown_args = true;

				let (attribute_vals, examples) = handle_special_attributes(attributes, working_set, &mut signature);

				let declaration = working_set.get_decl_mut(decl_id);

				if let Some(block_id) = body.and_then(|x| x.as_block()) {
					if signature.rest_positional.is_none() {
						working_set.error(ParseError::InternalError(
							"Extern block must have a rest positional argument".into(),
							name_expr.span,
						));
					} else {
						*declaration = signature.clone().into_block_command(block_id, attribute_vals, examples);

						working_set.get_block_mut(block_id).signature = signature;
					}
				} else {
					if signature.rest_positional.is_none() {
						// Make sure that a known external takes rest args with ExternalArgument
						// shape
						*signature = signature.rest("args", SyntaxShape::ExternalArgument, "all other arguments to the command");
					}

					let decl = KnownExternal {
						signature,
						attributes: attribute_vals,
						examples,
					};

					*declaration = Box::new(decl);
				}
			} else {
				working_set.error(ParseError::InternalError("Predeclaration failed to add declaration".into(), spans[split_id]));
			};
		}
		if let Some(name) = name_expr.as_string() {
			// It's OK if it returns None: The decl was already merged in previous parse pass.
			working_set.merge_predecl(name.as_bytes());
		} else {
			working_set.error(ParseError::UnknownState("Could not get string from string expression".into(), name_expr.span));
		}
	}

	Expression::new(working_set, Expr::Call(call), call_span, Type::Any)
}

fn handle_special_attributes(
	attributes: Vec<(String, Value)>,
	working_set: &mut StateWorkingSet<'_>,
	signature: &mut Signature,
) -> (Vec<(String, Value)>, Vec<CustomExample>) {
	let mut attribute_vals = vec![];
	let mut examples = vec![];
	let mut search_terms = vec![];
	let mut category = String::new();

	for (name, value) in attributes {
		let val_span = value.span();
		match name.as_str() {
			"example" => match CustomExample::from_value(value) {
				Ok(example) => examples.push(example),
				Err(_) => {
					let e = ShellError::GenericError {
						error: "nu::shell::invalid_example".into(),
						msg: "Value couldn't be converted to an example".into(),
						span: Some(val_span),
						help: Some("Is `attr example` shadowed?".into()),
						inner: vec![],
					};
					working_set.error(e.wrap(working_set, val_span));
				}
			},
			"search-terms" => match <Vec<String>>::from_value(value) {
				Ok(mut terms) => {
					search_terms.append(&mut terms);
				}
				Err(_) => {
					let e = ShellError::GenericError {
						error: "nu::shell::invalid_search_terms".into(),
						msg: "Value couldn't be converted to search-terms".into(),
						span: Some(val_span),
						help: Some("Is `attr search-terms` shadowed?".into()),
						inner: vec![],
					};
					working_set.error(e.wrap(working_set, val_span));
				}
			},
			"category" => match <String>::from_value(value) {
				Ok(term) => {
					category.push_str(&term);
				}
				Err(_) => {
					let e = ShellError::GenericError {
						error: "nu::shell::invalid_category".into(),
						msg: "Value couldn't be converted to category".into(),
						span: Some(val_span),
						help: Some("Is `attr category` shadowed?".into()),
						inner: vec![],
					};
					working_set.error(e.wrap(working_set, val_span));
				}
			},
			"complete" => match <Spanned<String>>::from_value(value) {
				Ok(Spanned { item, span }) => {
					if let Some(decl) = working_set.find_decl(item.as_bytes()) {
						// TODO: Enforce command signature? Not before settling on a unified
						// custom completion api
						signature.complete = Some(CommandWideCompleter::Command(decl));
					} else {
						working_set.error(ParseError::UnknownCommand(span));
					}
				}
				Err(_) => {
					let e = ShellError::GenericError {
						error: "nu::shell::invalid_completer".into(),
						msg: "Value couldn't be converted to a completer".into(),
						span: Some(val_span),
						help: Some("Is `attr complete` shadowed?".into()),
						inner: vec![],
					};
					working_set.error(e.wrap(working_set, val_span));
				}
			},
			"complete external" => match value {
				Value::Nothing { .. } => {
					signature.complete = Some(CommandWideCompleter::External);
				}
				_ => {
					let e = ShellError::GenericError {
						error: "nu::shell::invalid_completer".into(),
						msg: "This attribute shouldn't return anything".into(),
						span: Some(val_span),
						help: Some("Is `attr complete` shadowed?".into()),
						inner: vec![],
					};
					working_set.error(e.wrap(working_set, val_span));
				}
			},
			_ => {
				attribute_vals.push((name, value));
			}
		}
	}

	signature.search_terms = search_terms;
	signature.category = category_from_string(&category);

	(attribute_vals, examples)
}

fn check_alias_name<'a>(working_set: &mut StateWorkingSet, spans: &'a [Span]) -> Option<&'a Span> {
	let command_len = if !spans.is_empty() {
		if working_set.get_span_contents(spans[0]) == b"export" { 2 } else { 1 }
	} else {
		return None;
	};

	if spans.len() == command_len {
		None
	} else if spans.len() < command_len + 3 {
		if working_set.get_span_contents(spans[command_len]) == b"=" {
			let name = String::from_utf8_lossy(working_set.get_span_contents(Span::concat(&spans[..command_len])));
			working_set.error(ParseError::AssignmentMismatch(
				format!("{name} missing name"),
				"missing name".into(),
				spans[command_len],
			));
			Some(&spans[command_len])
		} else {
			None
		}
	} else if working_set.get_span_contents(spans[command_len + 1]) != b"=" {
		let name = String::from_utf8_lossy(working_set.get_span_contents(Span::concat(&spans[..command_len])));
		working_set.error(ParseError::AssignmentMismatch(
			format!("{name} missing sign"),
			"missing equal sign".into(),
			spans[command_len + 1],
		));
		Some(&spans[command_len + 1])
	} else {
		None
	}
}
