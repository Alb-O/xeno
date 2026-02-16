#[test]
fn shell_error_serialize_roundtrip() {
	// Ensure that we can serialize and deserialize `ShellError`, and check that it basically would
	// look the same
	let original_error = ShellError::CantConvert {
		span: Span::new(100, 200),
		to_type: "Foo".into(),
		from_type: "Bar".into(),
		help: Some("this is a test".into()),
	};
	println!("orig_error = {original_error:#?}");

	let serialized = serde_json::to_string_pretty(&original_error).expect("serde_json::to_string_pretty failed");
	println!("serialized = {serialized}");

	let deserialized: ShellError = serde_json::from_str(&serialized).expect("serde_json::from_str failed");
	println!("deserialized = {deserialized:#?}");

	// We don't expect the deserialized error to be the same as the original error, but its miette
	// properties should be comparable
	assert_eq!(original_error.to_string(), deserialized.to_string());

	assert_eq!(original_error.code().map(|c| c.to_string()), deserialized.code().map(|c| c.to_string()));

	let orig_labels = original_error.labels().into_iter().flatten().collect::<Vec<_>>();
	let deser_labels = deserialized.labels().into_iter().flatten().collect::<Vec<_>>();

	assert_eq!(orig_labels, deser_labels);

	assert_eq!(original_error.help().map(|c| c.to_string()), deserialized.help().map(|c| c.to_string()));
}

#[cfg(test)]
mod test {
	use super::*;

	impl From<std::io::Error> for ShellError {
		fn from(_: std::io::Error) -> ShellError {
			unimplemented!("This implementation is defined in the test module to ensure no other implementation exists.")
		}
	}

	impl From<Spanned<std::io::Error>> for ShellError {
		fn from(_: Spanned<std::io::Error>) -> Self {
			unimplemented!("This implementation is defined in the test module to ensure no other implementation exists.")
		}
	}

	impl From<ShellError> for std::io::Error {
		fn from(_: ShellError) -> Self {
			unimplemented!("This implementation is defined in the test module to ensure no other implementation exists.")
		}
	}
}
