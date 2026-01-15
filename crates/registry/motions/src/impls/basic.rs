use xeno_primitives::range::Direction;

use crate::motion;
use crate::movement::{move_horizontally, move_vertically};

motion!(left, { description: "Move left" }, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Backward, count, extend)
});

motion!(right, { description: "Move right" }, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Forward, count, extend)
});

motion!(up, { description: "Move up" }, |text, range, count, extend| {
	move_vertically(text, range, Direction::Backward, count, extend)
});

motion!(down, { description: "Move down" }, |text, range, count, extend| {
	move_vertically(text, range, Direction::Forward, count, extend)
});
