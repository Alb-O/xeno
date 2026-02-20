use super::*;

#[test]
fn test_delete_with_yank() {
	let op = delete(true);
	assert!(op.pre.contains(&PreEffect::Yank));
	assert!(matches!(op.transform, TextTransform::Delete));
}

#[test]
fn test_delete_without_yank() {
	let op = delete(false);
	assert!(!op.pre.contains(&PreEffect::Yank));
	assert!(matches!(op.transform, TextTransform::Delete));
}

#[test]
fn test_change_enters_insert_mode() {
	let op = change(false);
	assert!(matches!(op.transform, TextTransform::Delete));
	assert!(op.post.contains(&PostEffect::SetMode(Mode::Insert)));
}

#[test]
fn test_open_below_composition() {
	let op = open_below();
	assert!(matches!(op.selection, SelectionOp::ToLineEnd));
	assert!(matches!(op.transform, TextTransform::InsertNewlineWithIndent));
	assert!(op.post.contains(&PostEffect::SetMode(Mode::Insert)));
}

#[test]
fn test_char_map_lowercase() {
	let mapped: String = CharMapKind::ToLowerCase.apply('A').collect();
	assert_eq!(mapped, "a");
}

#[test]
fn test_char_map_swapcase() {
	let lower: String = CharMapKind::SwapCase.apply('A').collect();
	let upper: String = CharMapKind::SwapCase.apply('a').collect();
	assert_eq!(lower, "a");
	assert_eq!(upper, "A");
}
