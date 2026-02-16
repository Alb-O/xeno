pub fn parse_attribute(working_set: &mut StateWorkingSet, lite_command: &LiteCommand) -> (Attribute, Option<String>) {
	let _ = lite_command
		.parts
		.first()
		.filter(|s| working_set.get_span_contents(**s).starts_with(b"@"))
		.expect("Attributes always start with an `@`");

	assert!(lite_command.attribute_idx.is_empty(), "attributes can't have attributes");

	let mut spans = lite_command.parts.clone();
	if let Some(first) = spans.first_mut() {
		first.start += 1;
	}
	let spans = spans.as_slice();
	let attr_span = Span::concat(spans);

	let (cmd_start, cmd_end, mut name, decl_id) = find_longest_decl_with_prefix(working_set, spans, b"attr");

	debug_assert!(name.starts_with(b"attr "));
	let _ = name.drain(..(b"attr ".len()));

	let name_span = Span::concat(&spans[cmd_start..cmd_end]);

	let Ok(name) = String::from_utf8(name) else {
		working_set.error(ParseError::NonUtf8(name_span));
		return (
			Attribute {
				expr: garbage(working_set, attr_span),
			},
			None,
		);
	};

	let Some(decl_id) = decl_id else {
		working_set.error(ParseError::UnknownCommand(name_span));
		return (
			Attribute {
				expr: garbage(working_set, attr_span),
			},
			None,
		);
	};

	let decl = working_set.get_decl(decl_id);

	let parsed_call = match decl.as_alias() {
		// TODO: Once `const def` is available, we should either disallow aliases as attributes OR
		// allow them but rather than using the aliases' name, use the name of the aliased command
		Some(alias) => match &alias.clone().wrapped_call {
			Expression {
				expr: Expr::ExternalCall(..), ..
			} => {
				let shell_error = ShellError::NotAConstCommand { span: name_span };
				working_set.error(shell_error.wrap(working_set, attr_span));
				return (
					Attribute {
						expr: garbage(working_set, Span::concat(spans)),
					},
					None,
				);
			}
			_ => {
				trace!("parsing: alias of internal call");
				parse_internal_call(working_set, name_span, &spans[cmd_end..], decl_id, ArgumentParsingLevel::Full)
			}
		},
		None => {
			trace!("parsing: internal call");
			parse_internal_call(working_set, name_span, &spans[cmd_end..], decl_id, ArgumentParsingLevel::Full)
		}
	};

	(
		Attribute {
			expr: Expression::new(working_set, Expr::Call(parsed_call.call), Span::concat(spans), parsed_call.output),
		},
		Some(name),
	)
}
