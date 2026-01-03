use super::*;
use crate::buffer::Buffer;
use crate::layout::{Constraint, Rect};
use crate::widgets::StatefulWidget;
use crate::widgets::table::{Row, Table, TableState};

#[fixture]
fn table_buf() -> Buffer {
	Buffer::empty(Rect::new(0, 0, 10, 10))
}

#[rstest]
fn test_list_state_empty_list(mut table_buf: Buffer) {
	let mut state = TableState::default();

	let rows: Vec<Row> = Vec::new();
	let widths = vec![Constraint::Percentage(100)];
	let table = Table::new(rows, widths);
	state.select_first();
	StatefulWidget::render(table, table_buf.area, &mut table_buf, &mut state);
	assert_eq!(state.selected, None);
	assert_eq!(state.selected_column, None);
}

#[rstest]
fn test_list_state_single_item(mut table_buf: Buffer) {
	let mut state = TableState::default();

	let widths = vec![Constraint::Percentage(100)];

	let items = vec![Row::new(vec!["Item 1"])];
	let table = Table::new(items, widths);
	state.select_first();
	StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
	assert_eq!(state.selected, Some(0));
	assert_eq!(state.selected_column, None);

	state.select_last();
	StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
	assert_eq!(state.selected, Some(0));
	assert_eq!(state.selected_column, None);

	state.select_previous();
	StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
	assert_eq!(state.selected, Some(0));
	assert_eq!(state.selected_column, None);

	state.select_next();
	StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
	assert_eq!(state.selected, Some(0));
	assert_eq!(state.selected_column, None);

	let mut state = TableState::default();

	state.select_first_column();
	StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
	assert_eq!(state.selected_column, Some(0));
	assert_eq!(state.selected, None);

	state.select_last_column();
	StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
	assert_eq!(state.selected_column, Some(0));
	assert_eq!(state.selected, None);

	state.select_previous_column();
	StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
	assert_eq!(state.selected_column, Some(0));
	assert_eq!(state.selected, None);

	state.select_next_column();
	StatefulWidget::render(&table, table_buf.area, &mut table_buf, &mut state);
	assert_eq!(state.selected_column, Some(0));
	assert_eq!(state.selected, None);
}
