use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use ropey::Rope;
use tome_core::graphemes::{is_grapheme_boundary, next_grapheme_boundary, prev_grapheme_boundary};
use tome_core::movement::{
	move_horizontally, move_to_first_nonwhitespace, move_to_line_end, move_to_line_start,
	move_vertically,
};
use tome_core::range::Direction;
use tome_core::{Range, Selection, Transaction};

fn generate_text(lines: usize, chars_per_line: usize) -> String {
	let line: String = (0..chars_per_line)
		.map(|i| ((i % 26) as u8 + b'a') as char)
		.collect();
	(0..lines)
		.map(|_| line.as_str())
		.collect::<Vec<_>>()
		.join("\n")
}

fn generate_text_with_unicode(lines: usize) -> String {
	let patterns = [
		"Hello world ",
		"Cafe\u{301} ",
		"\u{1F600} emoji ",
		"\u{0915}\u{094D}\u{0937} ",
	];
	(0..lines)
		.map(|i| patterns[i % patterns.len()].repeat(10))
		.collect::<Vec<_>>()
		.join("\n")
}

// Document sizes to benchmark
const SMALL_LINES: usize = 100;
const MEDIUM_LINES: usize = 10_000;
const LARGE_LINES: usize = 100_000;
const GIGANTIC_LINES: usize = 1_000_000;
const CHARS_PER_LINE: usize = 80;

fn bench_rope_creation(c: &mut Criterion) {
	let mut group = c.benchmark_group("rope_creation");

	for (name, lines) in [
		("small_100", SMALL_LINES),
		("medium_10k", MEDIUM_LINES),
		("large_100k", LARGE_LINES),
		("gigantic_1M", GIGANTIC_LINES),
	] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let bytes = text.len();
		group.throughput(Throughput::Bytes(bytes as u64));
		group.bench_with_input(BenchmarkId::new("from_str", name), &text, |b, text| {
			b.iter(|| Rope::from(black_box(text.as_str())))
		});
	}

	group.finish();
}

fn bench_transaction_insert(c: &mut Criterion) {
	let mut group = c.benchmark_group("transaction_insert");

	for (name, lines) in [
		("small_100", SMALL_LINES),
		("medium_10k", MEDIUM_LINES),
		("large_100k", LARGE_LINES),
	] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let rope = Rope::from(text.as_str());
		let doc_len = rope.len_chars();

		group.bench_with_input(BenchmarkId::new("at_start", name), &rope, |b, rope| {
			b.iter(|| {
				let mut doc = rope.clone();
				let sel = Selection::point(0);
				let tx = Transaction::insert(doc.slice(..), &sel, "INSERTED TEXT".to_string());
				tx.apply(&mut doc);
				black_box(doc)
			})
		});

		let mid = doc_len / 2;
		group.bench_with_input(BenchmarkId::new("at_middle", name), &rope, |b, rope| {
			b.iter(|| {
				let mut doc = rope.clone();
				let sel = Selection::point(mid);
				let tx = Transaction::insert(doc.slice(..), &sel, "INSERTED TEXT".to_string());
				tx.apply(&mut doc);
				black_box(doc)
			})
		});

		group.bench_with_input(BenchmarkId::new("at_end", name), &rope, |b, rope| {
			b.iter(|| {
				let mut doc = rope.clone();
				let sel = Selection::point(doc_len);
				let tx = Transaction::insert(doc.slice(..), &sel, "INSERTED TEXT".to_string());
				tx.apply(&mut doc);
				black_box(doc)
			})
		});
	}

	group.finish();
}

fn bench_transaction_delete(c: &mut Criterion) {
	let mut group = c.benchmark_group("transaction_delete");

	for (name, lines) in [
		("small_100", SMALL_LINES),
		("medium_10k", MEDIUM_LINES),
		("large_100k", LARGE_LINES),
	] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let rope = Rope::from(text.as_str());
		let doc_len = rope.len_chars();
		let delete_size = 100.min(doc_len / 10);

		group.bench_with_input(BenchmarkId::new("at_start", name), &rope, |b, rope| {
			b.iter(|| {
				let mut doc = rope.clone();
				let sel = Selection::single(0, delete_size);
				let tx = Transaction::delete(doc.slice(..), &sel);
				tx.apply(&mut doc);
				black_box(doc)
			})
		});

		let mid = doc_len / 2;
		group.bench_with_input(BenchmarkId::new("at_middle", name), &rope, |b, rope| {
			b.iter(|| {
				let mut doc = rope.clone();
				let sel = Selection::single(mid, mid + delete_size);
				let tx = Transaction::delete(doc.slice(..), &sel);
				tx.apply(&mut doc);
				black_box(doc)
			})
		});

		group.bench_with_input(BenchmarkId::new("at_end", name), &rope, |b, rope| {
			b.iter(|| {
				let mut doc = rope.clone();
				let sel = Selection::single(doc_len - delete_size, doc_len);
				let tx = Transaction::delete(doc.slice(..), &sel);
				tx.apply(&mut doc);
				black_box(doc)
			})
		});
	}

	group.finish();
}

fn bench_transaction_multi_cursor(c: &mut Criterion) {
	let mut group = c.benchmark_group("transaction_multi_cursor");

	for (name, lines) in [
		("small_100", SMALL_LINES),
		("medium_10k", MEDIUM_LINES),
		("large_100k", LARGE_LINES),
	] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let rope = Rope::from(text.as_str());
		let doc_len = rope.len_chars();

		// Spread cursors across document
		for num_cursors in [10, 100, 1000] {
			if num_cursors > lines {
				continue;
			}

			let step = doc_len / num_cursors;
			let ranges: smallvec::SmallVec<[Range; 1]> =
				(0..num_cursors).map(|i| Range::point(i * step)).collect();
			let selection = Selection::new(ranges, 0);

			group.bench_with_input(
				BenchmarkId::new(format!("{}_cursors", num_cursors), name),
				&(rope.clone(), selection),
				|b, (rope, sel)| {
					b.iter(|| {
						let mut doc = rope.clone();
						let tx = Transaction::insert(doc.slice(..), sel, "X".to_string());
						tx.apply(&mut doc);
						black_box(doc)
					})
				},
			);
		}
	}

	group.finish();
}

fn bench_changeset_apply(c: &mut Criterion) {
	let mut group = c.benchmark_group("changeset_apply");

	for (name, lines) in [
		("small_100", SMALL_LINES),
		("medium_10k", MEDIUM_LINES),
		("large_100k", LARGE_LINES),
	] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let rope = Rope::from(text.as_str());
		let doc_len = rope.len_chars();

		// Multiple scattered changes
		let num_changes = 100.min(lines / 10).max(1);
		let step = doc_len / num_changes;
		let changes: Vec<_> = (0..num_changes)
			.map(|i| {
				let pos = i * step;
				(pos, pos + 1, Some("X".to_string()))
			})
			.collect();

		group.bench_with_input(
			BenchmarkId::new("scattered_100", name),
			&(rope.clone(), changes.clone()),
			|b, (rope, changes)| {
				b.iter(|| {
					let mut doc = rope.clone();
					let tx = Transaction::change(doc.slice(..), changes.clone());
					tx.apply(&mut doc);
					black_box(doc)
				})
			},
		);
	}

	group.finish();
}

fn bench_movement_horizontal(c: &mut Criterion) {
	let mut group = c.benchmark_group("movement_horizontal");

	for (name, lines) in [
		("small_100", SMALL_LINES),
		("medium_10k", MEDIUM_LINES),
		("large_100k", LARGE_LINES),
	] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let rope = Rope::from(text.as_str());
		let slice = rope.slice(..);
		let mid = rope.len_chars() / 2;
		let range = Range::point(mid);

		group.bench_with_input(BenchmarkId::new("forward_1", name), &slice, |b, slice| {
			b.iter(|| {
				black_box(move_horizontally(
					*slice,
					range,
					Direction::Forward,
					1,
					false,
				))
			})
		});

		group.bench_with_input(BenchmarkId::new("backward_1", name), &slice, |b, slice| {
			b.iter(|| {
				black_box(move_horizontally(
					*slice,
					range,
					Direction::Backward,
					1,
					false,
				))
			})
		});

		group.bench_with_input(BenchmarkId::new("forward_100", name), &slice, |b, slice| {
			b.iter(|| {
				black_box(move_horizontally(
					*slice,
					range,
					Direction::Forward,
					100,
					false,
				))
			})
		});
	}

	group.finish();
}

fn bench_movement_vertical(c: &mut Criterion) {
	let mut group = c.benchmark_group("movement_vertical");

	for (name, lines) in [
		("small_100", SMALL_LINES),
		("medium_10k", MEDIUM_LINES),
		("large_100k", LARGE_LINES),
	] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let rope = Rope::from(text.as_str());
		let slice = rope.slice(..);
		let mid = rope.len_chars() / 2;
		let range = Range::point(mid);

		group.bench_with_input(BenchmarkId::new("down_1", name), &slice, |b, slice| {
			b.iter(|| black_box(move_vertically(*slice, range, Direction::Forward, 1, false)))
		});

		group.bench_with_input(BenchmarkId::new("up_1", name), &slice, |b, slice| {
			b.iter(|| {
				black_box(move_vertically(
					*slice,
					range,
					Direction::Backward,
					1,
					false,
				))
			})
		});

		group.bench_with_input(BenchmarkId::new("down_100", name), &slice, |b, slice| {
			b.iter(|| {
				black_box(move_vertically(
					*slice,
					range,
					Direction::Forward,
					100,
					false,
				))
			})
		});

		group.bench_with_input(BenchmarkId::new("down_1000", name), &slice, |b, slice| {
			b.iter(|| {
				black_box(move_vertically(
					*slice,
					range,
					Direction::Forward,
					1000,
					false,
				))
			})
		});
	}

	group.finish();
}

fn bench_movement_line_ops(c: &mut Criterion) {
	let mut group = c.benchmark_group("movement_line_ops");

	for (name, lines) in [
		("small_100", SMALL_LINES),
		("medium_10k", MEDIUM_LINES),
		("large_100k", LARGE_LINES),
	] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let rope = Rope::from(text.as_str());
		let slice = rope.slice(..);
		let mid = rope.len_chars() / 2;
		let range = Range::point(mid);

		group.bench_with_input(
			BenchmarkId::new("to_line_start", name),
			&slice,
			|b, slice| b.iter(|| black_box(move_to_line_start(*slice, range, false))),
		);

		group.bench_with_input(BenchmarkId::new("to_line_end", name), &slice, |b, slice| {
			b.iter(|| black_box(move_to_line_end(*slice, range, false)))
		});

		group.bench_with_input(
			BenchmarkId::new("to_first_nonwhitespace", name),
			&slice,
			|b, slice| b.iter(|| black_box(move_to_first_nonwhitespace(*slice, range, false))),
		);
	}

	group.finish();
}

fn bench_selection_normalize(c: &mut Criterion) {
	let mut group = c.benchmark_group("selection_normalize");

	for num_ranges in [10, 100, 1000, 10000] {
		// Non-overlapping ranges
		let ranges: smallvec::SmallVec<[Range; 1]> = (0..num_ranges)
			.map(|i| Range::new(i * 100, i * 100 + 50))
			.collect();

		group.bench_with_input(
			BenchmarkId::new("non_overlapping", num_ranges),
			&ranges,
			|b, ranges| b.iter(|| black_box(Selection::new(ranges.clone(), 0))),
		);

		let overlapping: smallvec::SmallVec<[Range; 1]> = (0..num_ranges)
			.map(|i| Range::new(i * 10, i * 10 + 50))
			.collect();

		group.bench_with_input(
			BenchmarkId::new("overlapping", num_ranges),
			&overlapping,
			|b, ranges| b.iter(|| black_box(Selection::new(ranges.clone(), 0))),
		);
	}

	group.finish();
}

fn bench_selection_transform(c: &mut Criterion) {
	let mut group = c.benchmark_group("selection_transform");

	for num_ranges in [10, 100, 1000] {
		let ranges: smallvec::SmallVec<[Range; 1]> = (0..num_ranges)
			.map(|i| Range::new(i * 100, i * 100 + 50))
			.collect();
		let selection = Selection::new(ranges, 0);

		group.bench_with_input(
			BenchmarkId::new("shift_all", num_ranges),
			&selection,
			|b, sel| {
				b.iter(|| black_box(sel.transform(|r| Range::new(r.anchor + 10, r.head + 10))))
			},
		);
	}

	group.finish();
}

fn bench_grapheme_boundary(c: &mut Criterion) {
	let mut group = c.benchmark_group("grapheme_boundary");

	let ascii = generate_text(1000, 80);
	let ascii_rope = Rope::from(ascii.as_str());
	let ascii_slice = ascii_rope.slice(..);
	let ascii_mid = ascii_rope.len_chars() / 2;

	group.bench_function("is_boundary_ascii", |b| {
		b.iter(|| black_box(is_grapheme_boundary(ascii_slice, ascii_mid)))
	});

	group.bench_function("next_boundary_ascii", |b| {
		b.iter(|| black_box(next_grapheme_boundary(ascii_slice, ascii_mid)))
	});

	group.bench_function("prev_boundary_ascii", |b| {
		b.iter(|| black_box(prev_grapheme_boundary(ascii_slice, ascii_mid)))
	});

	// Unicode text with combining chars and emoji
	let unicode = generate_text_with_unicode(1000);
	let unicode_rope = Rope::from(unicode.as_str());
	let unicode_slice = unicode_rope.slice(..);
	let unicode_mid = unicode_rope.len_chars() / 2;

	group.bench_function("is_boundary_unicode", |b| {
		b.iter(|| black_box(is_grapheme_boundary(unicode_slice, unicode_mid)))
	});

	group.bench_function("next_boundary_unicode", |b| {
		b.iter(|| black_box(next_grapheme_boundary(unicode_slice, unicode_mid)))
	});

	group.bench_function("prev_boundary_unicode", |b| {
		b.iter(|| black_box(prev_grapheme_boundary(unicode_slice, unicode_mid)))
	});

	group.finish();
}

fn bench_map_selection(c: &mut Criterion) {
	let mut group = c.benchmark_group("map_selection");

	for (name, lines) in [
		("small_100", SMALL_LINES),
		("medium_10k", MEDIUM_LINES),
		("large_100k", LARGE_LINES),
	] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let rope = Rope::from(text.as_str());
		let doc_len = rope.len_chars();
		let mid = doc_len / 2;

		// Create a transaction with scattered inserts
		let num_changes = 100.min(lines / 10).max(1);
		let step = doc_len / num_changes;
		let changes: Vec<_> = (0..num_changes)
			.map(|i| {
				let pos = i * step;
				(pos, pos, Some("XXX".to_string()))
			})
			.collect();
		let tx = Transaction::change(rope.slice(..), changes);

		let single_sel = Selection::point(mid);
		group.bench_with_input(
			BenchmarkId::new("single_cursor", name),
			&(&tx, &single_sel),
			|b, (tx, sel)| b.iter(|| black_box(tx.map_selection(sel))),
		);

		let multi_ranges: smallvec::SmallVec<[Range; 1]> = (0..100.min(lines))
			.map(|i| Range::point(i * step.max(1)))
			.collect();
		let multi_sel = Selection::new(multi_ranges, 0);
		group.bench_with_input(
			BenchmarkId::new("100_cursors", name),
			&(&tx, &multi_sel),
			|b, (tx, sel)| b.iter(|| black_box(tx.map_selection(sel))),
		);
	}

	group.finish();
}

fn bench_changeset_compose(c: &mut Criterion) {
	let mut group = c.benchmark_group("changeset_compose");

	for (name, lines) in [("small_100", SMALL_LINES), ("medium_10k", MEDIUM_LINES)] {
		let text = generate_text(lines, CHARS_PER_LINE);
		let rope = Rope::from(text.as_str());

		let changes1: Vec<_> = (0..10)
			.map(|i| {
				let pos = i * 100;
				(pos, pos, Some("A".to_string()))
			})
			.collect();
		let tx1 = Transaction::change(rope.slice(..), changes1);

		// After tx1, document is longer
		let mut doc2 = rope.clone();
		tx1.apply(&mut doc2);

		let changes2: Vec<_> = (0..10)
			.map(|i| {
				let pos = i * 101 + 50;
				(pos, pos, Some("B".to_string()))
			})
			.collect();
		let tx2 = Transaction::change(doc2.slice(..), changes2);

		group.bench_with_input(
			BenchmarkId::new("compose_2", name),
			&(tx1.changes().clone(), tx2.changes().clone()),
			|b, (cs1, cs2)| b.iter(|| black_box(cs1.clone().compose(cs2.clone()))),
		);
	}

	group.finish();
}

criterion_group!(
	benches,
	bench_rope_creation,
	bench_transaction_insert,
	bench_transaction_delete,
	bench_transaction_multi_cursor,
	bench_changeset_apply,
	bench_movement_horizontal,
	bench_movement_vertical,
	bench_movement_line_ops,
	bench_selection_normalize,
	bench_selection_transform,
	bench_grapheme_boundary,
	bench_map_selection,
	bench_changeset_compose,
);

criterion_main!(benches);
