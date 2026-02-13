use std::collections::HashMap;

use xeno_language::LanguageLoader;
use xeno_language::syntax::{Syntax, SyntaxOptions};
use xeno_primitives::Transaction;
use xeno_primitives::transaction::Change;

use super::*;
use crate::syntax_manager::HighlightProjectionCtx;

fn default_style(_: &str) -> Style {
	Style::default()
}

fn parse_rust_syntax_for(rope: &Rope) -> (LanguageLoader, Syntax) {
	let loader = LanguageLoader::from_embedded();
	let lang = loader.language_for_name("rust").unwrap();
	let syntax = Syntax::new(rope.slice(..), lang, &loader, SyntaxOptions::default()).unwrap();
	(loader, syntax)
}

#[test]
fn test_highlight_tiles_new() {
	let cache = HighlightTiles::new();
	assert_eq!(cache.theme_epoch(), 0);
	assert!(cache.tiles.is_empty());
	assert!(cache.mru_order.is_empty());
	assert!(cache.projected_tiles.is_empty());
	assert!(cache.projected_index.is_empty());
}

#[test]
fn test_highlight_tiles_theme_epoch() {
	let mut cache = HighlightTiles::new();

	// Add a dummy tile
	cache.tiles.push(HighlightTile {
		key: HighlightKey {
			syntax_version: 1,
			theme_epoch: 0,
			language_id: None,
			tile_idx: 0,
		},
		spans: vec![],
	});
	cache.mru_order.push_back(0);
	cache.index.insert(DocumentId(1), {
		let mut m = HashMap::new();
		m.insert(0, 0);
		m
	});

	// Setting a new theme epoch should clear the cache
	cache.set_theme_epoch(1);
	assert_eq!(cache.theme_epoch(), 1);
	assert!(cache.tiles.is_empty());
	assert!(cache.mru_order.is_empty());
	assert!(cache.index.is_empty());
}

#[test]
fn test_highlight_tiles_lru_eviction() {
	let mut cache = HighlightTiles::with_capacity(2);
	let doc_id = DocumentId(1);

	// Insert first tile
	cache.insert_tile(
		doc_id,
		0,
		HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 0,
			},
			spans: vec![],
		},
	);

	// Insert second tile
	cache.insert_tile(
		doc_id,
		1,
		HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 1,
			},
			spans: vec![],
		},
	);

	// Both should exist
	assert_eq!(cache.tiles.len(), 2);
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&0));
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&1));

	// Insert third tile - should evict first (LRU)
	cache.insert_tile(
		doc_id,
		2,
		HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 2,
			},
			spans: vec![],
		},
	);

	// First should be evicted
	assert_eq!(cache.tiles.len(), 2);
	assert!(!cache.index.get(&doc_id).unwrap().contains_key(&0));
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&1));
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&2));
}

#[test]
fn test_highlight_tiles_mru_order() {
	let mut cache = HighlightTiles::with_capacity(2);
	let doc_id = DocumentId(1);

	// Insert tiles
	cache.insert_tile(
		doc_id,
		0,
		HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 0,
			},
			spans: vec![],
		},
	);
	cache.insert_tile(
		doc_id,
		1,
		HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 1,
			},
			spans: vec![],
		},
	);

	// Touch first tile (make it MRU)
	cache.touch(0);

	// Insert third tile - should evict second (now LRU)
	cache.insert_tile(
		doc_id,
		2,
		HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 2,
			},
			spans: vec![],
		},
	);

	assert!(cache.index.get(&doc_id).unwrap().contains_key(&0));
	assert!(!cache.index.get(&doc_id).unwrap().contains_key(&1));
	assert!(cache.index.get(&doc_id).unwrap().contains_key(&2));
}

#[test]
fn test_highlight_tiles_invalidate_document() {
	let mut cache = HighlightTiles::with_capacity(4);
	let doc1 = DocumentId(1);
	let doc2 = DocumentId(2);

	cache.insert_tile(
		doc1,
		0,
		HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 0,
			},
			spans: vec![],
		},
	);
	cache.insert_tile(
		doc1,
		1,
		HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 1,
			},
			spans: vec![],
		},
	);
	cache.insert_tile(
		doc2,
		0,
		HighlightTile {
			key: HighlightKey {
				syntax_version: 1,
				theme_epoch: 0,
				language_id: None,
				tile_idx: 0,
			},
			spans: vec![],
		},
	);

	assert_eq!(cache.index.len(), 2);

	cache.invalidate_document(doc1);

	assert!(!cache.index.contains_key(&doc1));
	assert!(cache.index.contains_key(&doc2));
}

#[test]
fn test_projected_tile_cache_reuses_same_target_version() {
	let mut cache = HighlightTiles::with_capacity(4);
	let doc_id = DocumentId(1);
	let key = HighlightKey {
		syntax_version: 1,
		theme_epoch: 0,
		language_id: None,
		tile_idx: 0,
	};
	cache.insert_tile(
		doc_id,
		0,
		HighlightTile {
			key,
			spans: vec![(
				HighlightSpan {
					start: 0,
					end: 3,
					highlight: xeno_language::highlight::Highlight::new(0),
				},
				Style::default(),
			)],
		},
	);

	let old_rope = Rope::from("XYZ");
	let tx = Transaction::change(
		old_rope.slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("abc".into()),
		}],
	);
	let mut new_rope = old_rope.clone();
	tx.apply(&mut new_rope);
	let (loader, syntax) = parse_rust_syntax_for(&old_rope);

	let projection = HighlightProjectionCtx {
		tree_doc_version: 1,
		target_doc_version: 2,
		base_rope: &old_rope,
		composed_changes: tx.changes(),
	};

	let q = HighlightSpanQuery {
		doc_id,
		syntax_version: 1,
		language_id: None,
		rope: &new_rope,
		syntax: &syntax,
		projection: Some(projection),
		language_loader: &loader,
		style_resolver: default_style,
		start_line: 0,
		end_line: new_rope.len_lines(),
	};
	let first = cache.get_or_build_projected_tile_index(&q, 0, key, projection);
	let second = cache.get_or_build_projected_tile_index(&q, 0, key, projection);

	assert_eq!(first, second);
	assert_eq!(cache.projected_tiles.len(), 1);
}

#[test]
fn test_projected_tile_cache_keys_target_doc_version() {
	let mut cache = HighlightTiles::with_capacity(4);
	let doc_id = DocumentId(1);
	let key = HighlightKey {
		syntax_version: 1,
		theme_epoch: 0,
		language_id: None,
		tile_idx: 0,
	};
	cache.insert_tile(
		doc_id,
		0,
		HighlightTile {
			key,
			spans: vec![(
				HighlightSpan {
					start: 0,
					end: 3,
					highlight: xeno_language::highlight::Highlight::new(0),
				},
				Style::default(),
			)],
		},
	);

	let old_rope = Rope::from("XYZ");
	let tx = Transaction::change(
		old_rope.slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("abc".into()),
		}],
	);
	let mut new_rope = old_rope.clone();
	tx.apply(&mut new_rope);
	let (loader, syntax) = parse_rust_syntax_for(&old_rope);

	let projection_v2 = HighlightProjectionCtx {
		tree_doc_version: 1,
		target_doc_version: 2,
		base_rope: &old_rope,
		composed_changes: tx.changes(),
	};
	let projection_v3 = HighlightProjectionCtx {
		tree_doc_version: 1,
		target_doc_version: 3,
		base_rope: &old_rope,
		composed_changes: tx.changes(),
	};

	let q_v2 = HighlightSpanQuery {
		doc_id,
		syntax_version: 1,
		language_id: None,
		rope: &new_rope,
		syntax: &syntax,
		projection: Some(projection_v2),
		language_loader: &loader,
		style_resolver: default_style,
		start_line: 0,
		end_line: new_rope.len_lines(),
	};
	let q_v3 = HighlightSpanQuery {
		doc_id,
		syntax_version: 1,
		language_id: None,
		rope: &new_rope,
		syntax: &syntax,
		projection: Some(projection_v3),
		language_loader: &loader,
		style_resolver: default_style,
		start_line: 0,
		end_line: new_rope.len_lines(),
	};
	let v2 = cache.get_or_build_projected_tile_index(&q_v2, 0, key, projection_v2);
	let v3 = cache.get_or_build_projected_tile_index(&q_v3, 0, key, projection_v3);

	assert_ne!(v2, v3);
	assert_eq!(cache.projected_tiles.len(), 2);
}

#[test]
fn test_projection_maps_target_tile_to_shifted_source_tile_range() {
	let mut cache = HighlightTiles::with_capacity(16);
	let doc_id = DocumentId(1);
	let syntax_version = 1;
	let key = HighlightKey {
		syntax_version,
		theme_epoch: 0,
		language_id: None,
		tile_idx: 0,
	};
	cache.insert_tile(
		doc_id,
		0,
		HighlightTile {
			key,
			spans: vec![(
				HighlightSpan {
					start: 0,
					end: 3,
					highlight: xeno_language::highlight::Highlight::new(0),
				},
				Style::default(),
			)],
		},
	);

	let old_rope = Rope::from("XYZ\n");
	let prefix = "\n".repeat(TILE_SIZE * 3);
	let tx = Transaction::change(
		old_rope.slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some(prefix.clone().into()),
		}],
	);
	let mut new_rope = old_rope.clone();
	tx.apply(&mut new_rope);
	let (loader, syntax) = parse_rust_syntax_for(&old_rope);

	let target_start_line = TILE_SIZE * 3;
	let target_end_line = target_start_line + 1;
	let projected_spans = cache.get_spans(HighlightSpanQuery {
		doc_id,
		syntax_version,
		language_id: None,
		rope: &new_rope,
		syntax: &syntax,
		projection: Some(HighlightProjectionCtx {
			tree_doc_version: 1,
			target_doc_version: 2,
			base_rope: &old_rope,
			composed_changes: tx.changes(),
		}),
		language_loader: &loader,
		style_resolver: default_style,
		start_line: target_start_line,
		end_line: target_end_line,
	});

	assert!(!projected_spans.is_empty(), "projected spans should be rebuilt from mapped source tiles");
	let inserted_bytes = prefix.len() as u32;
	assert!(
		projected_spans
			.iter()
			.any(|(span, _)| span.start <= inserted_bytes && span.end >= inserted_bytes + 3),
		"expected remapped span near shifted byte offset"
	);
}

#[test]
fn test_highlight_tiles_key_mismatch() {
	let mut cache = HighlightTiles::new();
	let doc_id = DocumentId(1);

	let old_key = HighlightKey {
		syntax_version: 1,
		theme_epoch: 0,
		language_id: None,
		tile_idx: 0,
	};

	let new_key = HighlightKey {
		syntax_version: 2, // Different!
		theme_epoch: 0,
		language_id: None,
		tile_idx: 0,
	};

	// Insert tile with old key
	cache.insert_tile(doc_id, 0, HighlightTile { key: old_key, spans: vec![] });

	// Getting with new key should return None (cache miss)
	assert!(cache.get_cached_tile(doc_id, 0, &new_key).is_none());

	// Getting with old key should return Some
	assert!(cache.get_cached_tile(doc_id, 0, &old_key).is_some());
}

#[test]
fn test_highlight_tiles_capacity_1_reuse_no_panic() {
	let mut cache = HighlightTiles::with_capacity(1);
	let doc_id = DocumentId(1);
	let key = HighlightKey {
		syntax_version: 1,
		theme_epoch: 0,
		language_id: None,
		tile_idx: 0,
	};

	// Insert first
	cache.insert_tile(doc_id, 0, HighlightTile { key, spans: vec![] });
	assert_eq!(cache.tiles.len(), 1);
	assert_eq!(cache.mru_order.len(), 1);

	// Insert again (reuse) - should not panic even with capacity 1
	cache.insert_tile(doc_id, 0, HighlightTile { key, spans: vec![] });
	assert_eq!(cache.tiles.len(), 1);
	assert_eq!(cache.mru_order.len(), 1);
}

#[test]
fn test_invalidate_then_evict_must_not_panic() {
	let mut cache = HighlightTiles::with_capacity(1);
	let doc1 = DocumentId(1);
	let doc2 = DocumentId(2);
	let key = HighlightKey {
		syntax_version: 1,
		theme_epoch: 0,
		language_id: None,
		tile_idx: 0,
	};

	// Fill to capacity
	cache.insert_tile(doc1, 0, HighlightTile { key, spans: vec![] });

	// Invalidate doc1
	cache.invalidate_document(doc1);

	// Insert doc2 - should evict doc1's tile normally
	cache.insert_tile(doc2, 0, HighlightTile { key, spans: vec![] });

	assert!(cache.index.contains_key(&doc2));
	assert!(!cache.index.contains_key(&doc1));
}

#[test]
fn test_remap_stale_span_tracks_delete_before_span() {
	let old_rope = Rope::from("abcdeXYZ");
	let tx = Transaction::change(
		old_rope.slice(..),
		[Change {
			start: 0,
			end: 5,
			replacement: None,
		}],
	);

	let mut new_rope = old_rope.clone();
	tx.apply(&mut new_rope);

	let span = HighlightSpan {
		start: 5,
		end: 8,
		highlight: xeno_language::highlight::Highlight::new(0),
	};

	let (start, end) = remap_stale_span_to_current(&span, &old_rope, &new_rope, tx.changes()).unwrap();
	assert_eq!(start, 0);
	assert_eq!(end, 3);
}

#[test]
fn test_remap_stale_span_tracks_insert_before_span() {
	let old_rope = Rope::from("XYZ");
	let tx = Transaction::change(
		old_rope.slice(..),
		[Change {
			start: 0,
			end: 0,
			replacement: Some("abc".into()),
		}],
	);

	let mut new_rope = old_rope.clone();
	tx.apply(&mut new_rope);

	let span = HighlightSpan {
		start: 0,
		end: 3,
		highlight: xeno_language::highlight::Highlight::new(0),
	};

	let (start, end) = remap_stale_span_to_current(&span, &old_rope, &new_rope, tx.changes()).unwrap();
	assert_eq!(start, 3);
	assert_eq!(end, 6);
}
