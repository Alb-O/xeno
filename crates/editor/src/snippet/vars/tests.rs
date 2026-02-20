use super::EditorSnippetResolver;
use crate::Editor;
use crate::snippet::SnippetVarResolver;

#[tokio::test(flavor = "current_thread")]
async fn current_time_variables_have_expected_shapes_and_ranges() {
	let editor = Editor::new_scratch();
	let resolver = EditorSnippetResolver::new(&editor, editor.focused_view());

	let year = resolver.resolve_var("CURRENT_YEAR").expect("CURRENT_YEAR should resolve");
	assert_eq!(year.len(), 4);
	assert!(year.chars().all(|ch| ch.is_ascii_digit()));

	let month = resolver.resolve_var("CURRENT_MONTH").expect("CURRENT_MONTH should resolve");
	assert_eq!(month.len(), 2);
	let month_num = month.parse::<u32>().expect("CURRENT_MONTH should be numeric");
	assert!((1..=12).contains(&month_num));

	let date = resolver.resolve_var("CURRENT_DATE").expect("CURRENT_DATE should resolve");
	assert_eq!(date.len(), 2);
	let date_num = date.parse::<u32>().expect("CURRENT_DATE should be numeric");
	assert!((1..=31).contains(&date_num));

	let hour = resolver.resolve_var("CURRENT_HOUR").expect("CURRENT_HOUR should resolve");
	assert_eq!(hour.len(), 2);
	let hour_num = hour.parse::<u32>().expect("CURRENT_HOUR should be numeric");
	assert!(hour_num <= 23);

	let minute = resolver.resolve_var("CURRENT_MINUTE").expect("CURRENT_MINUTE should resolve");
	assert_eq!(minute.len(), 2);
	let minute_num = minute.parse::<u32>().expect("CURRENT_MINUTE should be numeric");
	assert!(minute_num <= 59);

	let second = resolver.resolve_var("CURRENT_SECOND").expect("CURRENT_SECOND should resolve");
	assert_eq!(second.len(), 2);
	let second_num = second.parse::<u32>().expect("CURRENT_SECOND should be numeric");
	assert!(second_num <= 59);
}
