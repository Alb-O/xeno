use super::*;

#[test]
fn detect_addition() {
	assert_eq!(
		DiffLineType::from_line("+added line"),
		DiffLineType::Addition
	);
	assert_eq!(DiffLineType::from_line("+ "), DiffLineType::Addition);
}

#[test]
fn detect_deletion() {
	assert_eq!(
		DiffLineType::from_line("-removed line"),
		DiffLineType::Deletion
	);
	assert_eq!(DiffLineType::from_line("- "), DiffLineType::Deletion);
}

#[test]
fn detect_hunk() {
	assert_eq!(
		DiffLineType::from_line("@@ -1,3 +1,4 @@"),
		DiffLineType::Hunk
	);
}

#[test]
fn detect_context() {
	assert_eq!(
		DiffLineType::from_line(" context line"),
		DiffLineType::Context
	);
	assert_eq!(DiffLineType::from_line("plain line"), DiffLineType::Context);
}

#[test]
fn file_headers_are_context() {
	assert_eq!(DiffLineType::from_line("+++"), DiffLineType::Context);
	assert_eq!(
		DiffLineType::from_line("+++ a/file.rs"),
		DiffLineType::Context
	);
	assert_eq!(DiffLineType::from_line("---"), DiffLineType::Context);
	assert_eq!(
		DiffLineType::from_line("--- b/file.rs"),
		DiffLineType::Context
	);
}

#[test]
fn parse_hunk_header_standard() {
	let header = HunkHeader::parse("@@ -1,3 +1,4 @@").unwrap();
	assert_eq!(header.old_start, 1);
	assert_eq!(header.old_count, 3);
	assert_eq!(header.new_start, 1);
	assert_eq!(header.new_count, 4);
}

#[test]
fn parse_hunk_header_no_count() {
	let header = HunkHeader::parse("@@ -1 +1 @@").unwrap();
	assert_eq!(header.old_start, 1);
	assert_eq!(header.old_count, 1);
	assert_eq!(header.new_start, 1);
	assert_eq!(header.new_count, 1);
}

#[test]
fn parse_hunk_header_new_file() {
	let header = HunkHeader::parse("@@ -0,0 +1,5 @@").unwrap();
	assert_eq!(header.old_start, 0);
	assert_eq!(header.old_count, 0);
	assert_eq!(header.new_start, 1);
	assert_eq!(header.new_count, 5);
}

#[test]
fn parse_hunk_header_with_context() {
	// Some tools include function context after the @@ markers
	let header = HunkHeader::parse("@@ -10,7 +10,8 @@ fn main() {").unwrap();
	assert_eq!(header.old_start, 10);
	assert_eq!(header.old_count, 7);
	assert_eq!(header.new_start, 10);
	assert_eq!(header.new_count, 8);
}

#[test]
fn parse_hunk_header_invalid() {
	assert!(HunkHeader::parse("not a hunk").is_none());
	assert!(HunkHeader::parse("--- a/file.rs").is_none());
	assert!(HunkHeader::parse("+++ b/file.rs").is_none());
}

#[test]
fn compute_line_numbers_simple() {
	let diff = r#"diff --git a/file.rs b/file.rs
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,4 @@
 context
+added
 context
 context
"#;
	let rope = Rope::from_str(diff);
	let nums = compute_diff_line_numbers(&rope);

	// Lines 0-2: file headers (no line numbers)
	assert_eq!(nums[0], DiffLineNumbers::default());
	assert_eq!(nums[1], DiffLineNumbers::default());
	assert_eq!(nums[2], DiffLineNumbers::default());

	// Line 3: hunk header (no line numbers)
	assert_eq!(nums[3], DiffLineNumbers::default());

	// Line 4: context (old:1, new:1)
	assert_eq!(
		nums[4],
		DiffLineNumbers {
			old: Some(1),
			new: Some(1)
		}
	);

	// Line 5: addition (new:2 only)
	assert_eq!(
		nums[5],
		DiffLineNumbers {
			old: None,
			new: Some(2)
		}
	);

	// Line 6: context (old:2, new:3)
	assert_eq!(
		nums[6],
		DiffLineNumbers {
			old: Some(2),
			new: Some(3)
		}
	);

	// Line 7: context (old:3, new:4)
	assert_eq!(
		nums[7],
		DiffLineNumbers {
			old: Some(3),
			new: Some(4)
		}
	);
}

#[test]
fn compute_line_numbers_deletion() {
	let diff = r#"@@ -1,3 +1,2 @@
 context
-deleted
 context
"#;
	let rope = Rope::from_str(diff);
	let nums = compute_diff_line_numbers(&rope);

	// Line 0: hunk header
	assert_eq!(nums[0], DiffLineNumbers::default());

	// Line 1: context (old:1, new:1)
	assert_eq!(
		nums[1],
		DiffLineNumbers {
			old: Some(1),
			new: Some(1)
		}
	);

	// Line 2: deletion (old:2 only)
	assert_eq!(
		nums[2],
		DiffLineNumbers {
			old: Some(2),
			new: None
		}
	);

	// Line 3: context (old:3, new:2)
	assert_eq!(
		nums[3],
		DiffLineNumbers {
			old: Some(3),
			new: Some(2)
		}
	);
}
