use super::Dirty;

#[test]
fn full_implies_redraw_and_is_superset() {
	assert!(Dirty::FULL.needs_redraw());
	assert_eq!(Dirty::FULL | Dirty::REDRAW, Dirty::FULL);
}
