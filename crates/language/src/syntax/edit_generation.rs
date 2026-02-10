use ropey::RopeSlice;
use tree_house::tree_sitter::{InputEdit, Point};
use xeno_primitives::transaction::Operation;

/// Generates tree-sitter InputEdits from a Xeno ChangeSet.
pub(super) fn generate_edits(
	old_text: RopeSlice,
	changeset: &xeno_primitives::ChangeSet,
) -> Vec<InputEdit> {
	fn add_delta(start: Point, text: &str) -> Point {
		let bytes = text.as_bytes();
		let mut row = start.row;
		let mut col = start.col;
		for &b in bytes {
			if b == b'\n' {
				row += 1;
				col = 0;
			} else {
				col += 1;
			}
		}
		Point { row, col }
	}

	fn add_delta_rope(start: Point, rope: RopeSlice) -> Point {
		let mut p = start;
		for chunk in rope.chunks() {
			p = add_delta(p, chunk);
		}
		p
	}

	let mut edits = Vec::new();
	let mut old_pos = 0usize;
	let mut current_byte = 0u32;
	let mut current_point = Point { row: 0, col: 0 };

	if changeset.is_empty() {
		return edits;
	}

	let mut iter = changeset.changes().iter().peekable();

	while let Some(change) = iter.next() {
		match change {
			Operation::Retain(len) => {
				let segment = old_text.slice(old_pos..old_pos + len);
				current_byte += segment.len_bytes() as u32;
				current_point = add_delta_rope(current_point, segment);
				old_pos += len;
			}
			Operation::Delete(len) => {
				let start_byte = current_byte;
				let start_point = current_point;

				let segment = old_text.slice(old_pos..old_pos + len);
				let old_end_byte = start_byte + segment.len_bytes() as u32;
				let old_end_point = add_delta_rope(start_point, segment);

				edits.push(InputEdit {
					start_byte,
					old_end_byte,
					new_end_byte: start_byte,
					start_point,
					old_end_point,
					new_end_point: start_point,
				});
				old_pos += len;
			}
			Operation::Insert(s) => {
				let start_byte = current_byte;
				let start_point = current_point;

				let insert_len = s.byte_len() as u32;
				let new_end_point = add_delta(start_point, s.text());

				// Check for subsequent delete (replacement)
				if let Some(Operation::Delete(del_len)) = iter.peek() {
					let del_segment = old_text.slice(old_pos..old_pos + del_len);
					let old_end_byte = start_byte + del_segment.len_bytes() as u32;
					let old_end_point = add_delta_rope(start_point, del_segment);
					iter.next();
					old_pos += del_len;

					edits.push(InputEdit {
						start_byte,
						old_end_byte,
						new_end_byte: start_byte + insert_len,
						start_point,
						old_end_point,
						new_end_point,
					});
				} else {
					edits.push(InputEdit {
						start_byte,
						old_end_byte: start_byte,
						new_end_byte: start_byte + insert_len,
						start_point,
						old_end_point: start_point,
						new_end_point,
					});
				}
				current_byte += insert_len;
				current_point = new_end_point;
			}
		}
	}

	edits
}
