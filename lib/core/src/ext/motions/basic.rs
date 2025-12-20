use crate::motion;
use crate::movement::move_horizontally;
use crate::range::Direction;

motion!(move_left, "Move left", |text, range, count, extend| {
	move_horizontally(text, range, Direction::Backward, count, extend)
});

motion!(move_right, "Move right", |text, range, count, extend| {
	move_horizontally(text, range, Direction::Forward, count, extend)
});

motion!(move_up, "Move up", |text, range, count, extend| {
	crate::movement::move_vertically(text, range, Direction::Backward, count, extend)
});

motion!(move_down, "Move down", |text, range, count, extend| {
	crate::movement::move_vertically(text, range, Direction::Forward, count, extend)
});
