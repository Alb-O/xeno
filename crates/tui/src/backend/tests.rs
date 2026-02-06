use super::*;

#[test]
fn clear_type_tostring() {
	assert_eq!(ClearType::All.to_string(), "All");
	assert_eq!(ClearType::AfterCursor.to_string(), "AfterCursor");
	assert_eq!(ClearType::BeforeCursor.to_string(), "BeforeCursor");
	assert_eq!(ClearType::CurrentLine.to_string(), "CurrentLine");
	assert_eq!(ClearType::UntilNewLine.to_string(), "UntilNewLine");
}

#[test]
fn clear_type_from_str() {
	assert_eq!("All".parse::<ClearType>(), Ok(ClearType::All));
	assert_eq!(
		"AfterCursor".parse::<ClearType>(),
		Ok(ClearType::AfterCursor)
	);
	assert_eq!(
		"BeforeCursor".parse::<ClearType>(),
		Ok(ClearType::BeforeCursor)
	);
	assert_eq!(
		"CurrentLine".parse::<ClearType>(),
		Ok(ClearType::CurrentLine)
	);
	assert_eq!(
		"UntilNewLine".parse::<ClearType>(),
		Ok(ClearType::UntilNewLine)
	);
	assert!("".parse::<ClearType>().is_err());
}
