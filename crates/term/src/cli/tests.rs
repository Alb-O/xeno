use super::*;

#[test]
fn parse_plain_path() {
	let loc = FileLocation::parse("foo/bar.txt");
	assert_eq!(loc.path, PathBuf::from("foo/bar.txt"));
	assert_eq!(loc.line, None);
	assert_eq!(loc.column, None);
}

#[test]
fn parse_path_with_line() {
	let loc = FileLocation::parse("foo/bar.txt:42");
	assert_eq!(loc.path, PathBuf::from("foo/bar.txt"));
	assert_eq!(loc.line, Some(41)); // 0-indexed
	assert_eq!(loc.column, None);
}

#[test]
fn parse_path_with_line_and_column() {
	let loc = FileLocation::parse("foo/bar.txt:42:10");
	assert_eq!(loc.path, PathBuf::from("foo/bar.txt"));
	assert_eq!(loc.line, Some(41)); // 0-indexed
	assert_eq!(loc.column, Some(9)); // 0-indexed
}

#[test]
fn parse_line_one_is_zero_indexed() {
	let loc = FileLocation::parse("file.txt:1");
	assert_eq!(loc.line, Some(0));
}

#[test]
fn parse_line_zero_is_treated_as_plain_path() {
	// Line 0 is invalid (1-indexed input), so treat as plain path
	let loc = FileLocation::parse("file.txt:0");
	assert_eq!(loc.path, PathBuf::from("file.txt:0"));
	assert_eq!(loc.line, None);
}

#[test]
fn parse_non_numeric_suffix_is_plain_path() {
	let loc = FileLocation::parse("file.txt:abc");
	assert_eq!(loc.path, PathBuf::from("file.txt:abc"));
	assert_eq!(loc.line, None);
}

#[test]
fn parse_trailing_colon_is_plain_path() {
	let loc = FileLocation::parse("file.txt:");
	assert_eq!(loc.path, PathBuf::from("file.txt:"));
	assert_eq!(loc.line, None);
}

#[test]
fn parse_absolute_path_with_line() {
	let loc = FileLocation::parse("/home/user/file.txt:100");
	assert_eq!(loc.path, PathBuf::from("/home/user/file.txt"));
	assert_eq!(loc.line, Some(99));
}

#[test]
#[cfg(windows)]
fn parse_windows_path_with_line() {
	let loc = FileLocation::parse("C:\\Users\\test\\file.txt:42");
	assert_eq!(loc.path, PathBuf::from("C:\\Users\\test\\file.txt"));
	assert_eq!(loc.line, Some(41));
}
