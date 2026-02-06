use pretty_assertions::assert_eq;

use crate::widgets::list::ListState;

#[test]
fn selected() {
	let mut state = ListState::default();
	assert_eq!(state.selected(), None);

	state.select(Some(1));
	assert_eq!(state.selected(), Some(1));

	state.select(None);
	assert_eq!(state.selected(), None);
}

#[test]
fn select() {
	let mut state = ListState::default();
	assert_eq!(state.selected, None);
	assert_eq!(state.offset, 0);

	state.select(Some(2));
	assert_eq!(state.selected, Some(2));
	assert_eq!(state.offset, 0);

	state.select(None);
	assert_eq!(state.selected, None);
	assert_eq!(state.offset, 0);
}

#[test]
fn state_navigation() {
	let mut state = ListState::default();
	state.select_first();
	assert_eq!(state.selected, Some(0));

	state.select_previous();
	assert_eq!(state.selected, Some(0));

	state.select_next();
	assert_eq!(state.selected, Some(1));

	state.select_previous();
	assert_eq!(state.selected, Some(0));

	state.select_last();
	assert_eq!(state.selected, Some(usize::MAX));

	state.select_next(); // should not go above usize::MAX
	assert_eq!(state.selected, Some(usize::MAX));

	state.select_previous();
	assert_eq!(state.selected, Some(usize::MAX - 1));

	state.select_next();
	assert_eq!(state.selected, Some(usize::MAX));

	let mut state = ListState::default();
	state.select_next();
	assert_eq!(state.selected, Some(0));

	let mut state = ListState::default();
	state.select_previous();
	assert_eq!(state.selected, Some(usize::MAX));

	let mut state = ListState::default();
	state.select(Some(2));
	state.scroll_down_by(4);
	assert_eq!(state.selected, Some(6));

	let mut state = ListState::default();
	state.scroll_up_by(3);
	assert_eq!(state.selected, Some(0));

	state.select(Some(6));
	state.scroll_up_by(4);
	assert_eq!(state.selected, Some(2));

	state.scroll_up_by(4);
	assert_eq!(state.selected, Some(0));
}
