use super::*;

#[test]
fn fill_from_option() {
	let fill: FillConfig = Some(Color::Red).into();
	assert!(fill.fill_span(5).is_some());

	let fill: FillConfig = None.into();
	assert!(fill.fill_span(5).is_none());
}
