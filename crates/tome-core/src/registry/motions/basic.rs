use crate::motion;
use crate::movement::move_horizontally;
use crate::range::Direction;

motion!(move_left, { description: "Move left" }, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Backward, count, extend)
});

motion!(move_right, { description: "Move right" }, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Forward, count, extend)
});

motion!(move_up, { description: "Move up" }, |text, range, count, extend| {
	crate::movement::move_vertically(text, range, Direction::Backward, count, extend)
});

motion!(move_down, { description: "Move down" }, |text, range, count, extend| {
	crate::movement::move_vertically(text, range, Direction::Forward, count, extend)
});
