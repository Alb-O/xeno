use super::*;

#[test]
fn new() {
	let state = TableState::new();
	assert_eq!(state.offset, 0);
	assert_eq!(state.selected, None);
	assert_eq!(state.selected_column, None);
}

#[test]
fn with_offset() {
	let state = TableState::new().with_offset(1);
	assert_eq!(state.offset, 1);
}

#[test]
fn with_selected() {
	let state = TableState::new().with_selected(Some(1));
	assert_eq!(state.selected, Some(1));
}

#[test]
fn with_selected_column() {
	let state = TableState::new().with_selected_column(Some(1));
	assert_eq!(state.selected_column, Some(1));
}

#[test]
fn with_selected_cell_none() {
	let state = TableState::new().with_selected_cell(None);
	assert_eq!(state.selected, None);
	assert_eq!(state.selected_column, None);
}

#[test]
fn offset() {
	let state = TableState::new();
	assert_eq!(state.offset(), 0);
}

#[test]
fn offset_mut() {
	let mut state = TableState::new();
	*state.offset_mut() = 1;
	assert_eq!(state.offset, 1);
}

#[test]
fn selected() {
	let state = TableState::new();
	assert_eq!(state.selected(), None);
}

#[test]
fn selected_column() {
	let state = TableState::new();
	assert_eq!(state.selected_column(), None);
}

#[test]
fn selected_cell() {
	let state = TableState::new();
	assert_eq!(state.selected_cell(), None);
}

#[test]
fn selected_mut() {
	let mut state = TableState::new();
	*state.selected_mut() = Some(1);
	assert_eq!(state.selected, Some(1));
}

#[test]
fn selected_column_mut() {
	let mut state = TableState::new();
	*state.selected_column_mut() = Some(1);
	assert_eq!(state.selected_column, Some(1));
}

#[test]
fn select() {
	let mut state = TableState::new();
	state.select(Some(1));
	assert_eq!(state.selected, Some(1));
}

#[test]
fn select_none() {
	let mut state = TableState::new().with_selected(Some(1));
	state.select(None);
	assert_eq!(state.selected, None);
}

#[test]
fn select_column() {
	let mut state = TableState::new();
	state.select_column(Some(1));
	assert_eq!(state.selected_column, Some(1));
}

#[test]
fn select_column_none() {
	let mut state = TableState::new().with_selected_column(Some(1));
	state.select_column(None);
	assert_eq!(state.selected_column, None);
}

#[test]
fn select_cell() {
	let mut state = TableState::new();
	state.select_cell(Some((1, 5)));
	assert_eq!(state.selected_cell(), Some((1, 5)));
}

#[test]
fn select_cell_none() {
	let mut state = TableState::new().with_selected_cell(Some((1, 5)));
	state.select_cell(None);
	assert_eq!(state.selected, None);
	assert_eq!(state.selected_column, None);
	assert_eq!(state.selected_cell(), None);
}

#[test]
fn test_table_state_navigation() {
	let mut state = TableState::default();
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

	let mut state = TableState::default();
	state.select_next();
	assert_eq!(state.selected, Some(0));

	let mut state = TableState::default();
	state.select_previous();
	assert_eq!(state.selected, Some(usize::MAX));

	let mut state = TableState::default();
	state.select(Some(2));
	state.scroll_down_by(4);
	assert_eq!(state.selected, Some(6));

	let mut state = TableState::default();
	state.scroll_up_by(3);
	assert_eq!(state.selected, Some(0));

	state.select(Some(6));
	state.scroll_up_by(4);
	assert_eq!(state.selected, Some(2));

	state.scroll_up_by(4);
	assert_eq!(state.selected, Some(0));

	let mut state = TableState::default();
	state.select_first_column();
	assert_eq!(state.selected_column, Some(0));

	state.select_previous_column();
	assert_eq!(state.selected_column, Some(0));

	state.select_next_column();
	assert_eq!(state.selected_column, Some(1));

	state.select_previous_column();
	assert_eq!(state.selected_column, Some(0));

	state.select_last_column();
	assert_eq!(state.selected_column, Some(usize::MAX));

	state.select_previous_column();
	assert_eq!(state.selected_column, Some(usize::MAX - 1));

	let mut state = TableState::default().with_selected_column(Some(12));
	state.scroll_right_by(4);
	assert_eq!(state.selected_column, Some(16));

	state.scroll_left_by(20);
	assert_eq!(state.selected_column, Some(0));

	state.scroll_right_by(100);
	assert_eq!(state.selected_column, Some(100));

	state.scroll_left_by(20);
	assert_eq!(state.selected_column, Some(80));
}
