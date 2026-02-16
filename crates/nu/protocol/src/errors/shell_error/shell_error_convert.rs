impl From<Box<dyn std::error::Error>> for ShellError {
	fn from(error: Box<dyn std::error::Error>) -> ShellError {
		ShellError::GenericError {
			error: format!("{error:?}"),
			msg: error.to_string(),
			span: None,
			help: None,
			inner: vec![],
		}
	}
}

impl From<Box<dyn std::error::Error + Send + Sync>> for ShellError {
	fn from(error: Box<dyn std::error::Error + Send + Sync>) -> ShellError {
		ShellError::GenericError {
			error: format!("{error:?}"),
			msg: error.to_string(),
			span: None,
			help: None,
			inner: vec![],
		}
	}
}

impl From<super::LabeledError> for ShellError {
	fn from(error: super::LabeledError) -> Self {
		ShellError::LabeledError(Box::new(error))
	}
}

/// `ShellError` always serializes as [`LabeledError`].
impl Serialize for ShellError {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		LabeledError::from_diagnostic(self).serialize(serializer)
	}
}

/// `ShellError` always deserializes as if it were [`LabeledError`], resulting in a
/// [`ShellError::LabeledError`] variant.
impl<'de> Deserialize<'de> for ShellError {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		LabeledError::deserialize(deserializer).map(ShellError::from)
	}
}
