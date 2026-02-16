impl ShellError {
	pub fn external_exit_code(&self) -> Option<Spanned<i32>> {
		let (item, span) = match *self {
			Self::NonZeroExitCode { exit_code, span } => (exit_code.into(), span),
			#[cfg(unix)]
			Self::TerminatedBySignal { signal, span, .. } | Self::CoreDumped { signal, span, .. } => (-signal, span),
			_ => return None,
		};
		Some(Spanned { item, span })
	}

	pub fn exit_code(&self) -> Option<i32> {
		match self {
			Self::Return { .. } | Self::Break { .. } | Self::Continue { .. } => None,
			_ => self.external_exit_code().map(|e| e.item).or(Some(1)),
		}
	}

	pub fn into_full_value(self, working_set: &StateWorkingSet, stack: &Stack, span: Span) -> Value {
		let exit_code = self.external_exit_code();

		let mut record = record! {
			"msg" => Value::string(self.to_string(), span),
			"debug" => Value::string(format!("{self:?}"), span),
			"raw" => Value::error(self.clone(), span),
			"rendered" => Value::string(format_cli_error(Some(stack), working_set, &self, Some("nu::shell::error")), span),
			"json" => Value::string(serde_json::to_string(&self).expect("Could not serialize error"), span),
		};

		if let Some(code) = exit_code {
			record.push("exit_code", Value::int(code.item.into(), code.span));
		}

		Value::record(record, span)
	}

	// TODO: Implement as From trait
	pub fn wrap(self, working_set: &StateWorkingSet, span: Span) -> ParseError {
		let msg = format_cli_error(None, working_set, &self, None);
		ParseError::LabeledError(msg, "Encountered error during parse-time evaluation".into(), span)
	}

	/// Convert self error to a [`ShellError::ChainedError`] variant.
	pub fn into_chained(self, span: Span) -> Self {
		Self::ChainedError(match self {
			Self::ChainedError(inner) => ChainedError::new_chained(inner, span),
			other => {
				// If it's not already a chained error, it could have more errors below
				// it that we want to chain together
				let error = other.clone();
				let mut now = ChainedError::new(other, span);
				if let Some(related) = error.related() {
					let mapped = related
						.map(|s| {
							let shellerror: Self = Self::from_diagnostic(s);
							shellerror
						})
						.collect::<Vec<_>>();
					if !mapped.is_empty() {
						now.sources = [now.sources, mapped].concat();
					};
				}
				now
			}
		})
	}

	pub fn from_diagnostic(diag: &(impl miette::Diagnostic + ?Sized)) -> Self {
		Self::LabeledError(LabeledError::from_diagnostic(diag).into())
	}
}

impl FromValue for ShellError {
	fn from_value(v: Value) -> Result<Self, ShellError> {
		let from_type = v.get_type();
		match v {
			Value::Error { error, .. } => Ok(*error),
			// Also let it come from the into_full_value record.
			Value::Record { val, internal_span, .. } => Self::from_value(
				(*val)
					.get("raw")
					.ok_or(ShellError::CantConvert {
						to_type: Self::expected_type().to_string(),
						from_type: from_type.to_string(),
						span: internal_span,
						help: None,
					})?
					.clone(),
			),
			Value::Nothing { internal_span } => Ok(Self::GenericError {
				error: "error".into(),
				msg: "is nothing".into(),
				span: Some(internal_span),
				help: None,
				inner: vec![],
			}),
			_ => Err(ShellError::CantConvert {
				to_type: Self::expected_type().to_string(),
				from_type: v.get_type().to_string(),
				span: v.span(),
				help: None,
			}),
		}
	}
}
