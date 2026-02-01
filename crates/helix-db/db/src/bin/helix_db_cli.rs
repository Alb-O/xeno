use std::sync::Arc;

use bumpalo::Bump;
use helix_db::helix_engine::storage_core::HelixGraphStorage;
use helix_db::helix_engine::traversal_core::config::Config;
use helix_db::helix_engine::traversal_core::ops::g::G;
use helix_db::helix_engine::traversal_core::ops::in_::in_::InAdapter;
use helix_db::helix_engine::traversal_core::ops::out::out::OutAdapter;
use helix_db::helix_engine::traversal_core::ops::source::add_e::AddEAdapter;
use helix_db::helix_engine::traversal_core::ops::source::add_n::AddNAdapter;
use helix_db::helix_engine::traversal_core::ops::source::n_from_id::NFromIdAdapter;
use helix_db::helix_engine::traversal_core::ops::source::n_from_type::NFromTypeAdapter;
use helix_db::helix_engine::traversal_core::traversal_value::TraversalValue;
use helix_db::props;
use helix_db::protocol::value::Value;
use helix_db::utils::properties::ImmutablePropertiesMap;

fn make_props<'arena>(
	arena: &'arena Bump,
	props: Vec<(String, Value)>,
) -> Option<ImmutablePropertiesMap<'arena>> {
	let len = props.len();
	Some(ImmutablePropertiesMap::new(
		len,
		props.into_iter().map(|(key, value)| {
			let key: &'arena str = arena.alloc_str(&key);
			(key, value)
		}),
		arena,
	))
}

fn main() {
	let db_path = std::env::args()
		.nth(1)
		.unwrap_or_else(|| "./helix_test_db".to_string());

	println!("Creating database at: {db_path}");

	let storage = Arc::new(
		HelixGraphStorage::new(&db_path, Config::default(), Default::default())
			.expect("failed to create database"),
	);

	println!("Database created successfully.\n");

	// --- Create nodes ---
	let arena = Bump::new();
	let mut txn = storage
		.graph_env
		.write_txn()
		.expect("failed to start write txn");

	println!("Adding nodes...");

	let alice_nodes: Vec<_> = G::new_mut(&storage, &arena, &mut txn)
		.add_n(
			arena.alloc_str("person"),
			make_props(&arena, props! { "name" => "Alice", "age" => 30_i64 }),
			None,
		)
		.filter_map(|r| r.ok())
		.collect();

	let alice_id = alice_nodes[0].id();
	println!("  Created Alice (id: {})", uuid::Uuid::from_u128(alice_id));

	let bob_nodes: Vec<_> = G::new_mut(&storage, &arena, &mut txn)
		.add_n(
			arena.alloc_str("person"),
			make_props(&arena, props! { "name" => "Bob", "age" => 25_i64 }),
			None,
		)
		.filter_map(|r| r.ok())
		.collect();

	let bob_id = bob_nodes[0].id();
	println!("  Created Bob   (id: {})", uuid::Uuid::from_u128(bob_id));

	let charlie_nodes: Vec<_> = G::new_mut(&storage, &arena, &mut txn)
		.add_n(
			arena.alloc_str("person"),
			make_props(&arena, props! { "name" => "Charlie", "age" => 35_i64 }),
			None,
		)
		.filter_map(|r| r.ok())
		.collect();

	let charlie_id = charlie_nodes[0].id();
	println!(
		"  Created Charlie (id: {})",
		uuid::Uuid::from_u128(charlie_id)
	);

	let rust_nodes: Vec<_> = G::new_mut(&storage, &arena, &mut txn)
		.add_n(
			arena.alloc_str("language"),
			make_props(&arena, props! { "name" => "Rust", "year" => 2010_i64 }),
			None,
		)
		.filter_map(|r| r.ok())
		.collect();

	let rust_id = rust_nodes[0].id();
	println!("  Created Rust  (id: {})", uuid::Uuid::from_u128(rust_id));

	// --- Create edges ---
	println!("\nAdding edges...");

	G::new_mut(&storage, &arena, &mut txn)
		.add_edge(
			arena.alloc_str("knows"),
			None,
			alice_id,
			bob_id,
			false,
			false,
		)
		.collect::<Result<Vec<_>, _>>()
		.expect("failed to add Alice->knows->Bob");
	println!("  Alice --knows--> Bob");

	G::new_mut(&storage, &arena, &mut txn)
		.add_edge(
			arena.alloc_str("knows"),
			None,
			bob_id,
			charlie_id,
			false,
			false,
		)
		.collect::<Result<Vec<_>, _>>()
		.expect("failed to add Bob->knows->Charlie");
	println!("  Bob --knows--> Charlie");

	G::new_mut(&storage, &arena, &mut txn)
		.add_edge(
			arena.alloc_str("uses"),
			None,
			alice_id,
			rust_id,
			false,
			false,
		)
		.collect::<Result<Vec<_>, _>>()
		.expect("failed to add Alice->uses->Rust");
	println!("  Alice --uses--> Rust");

	G::new_mut(&storage, &arena, &mut txn)
		.add_edge(arena.alloc_str("uses"), None, bob_id, rust_id, false, false)
		.collect::<Result<Vec<_>, _>>()
		.expect("failed to add Bob->uses->Rust");
	println!("  Bob --uses--> Rust");

	txn.commit().expect("failed to commit write txn");
	println!("\nAll writes committed.");

	// --- Read back ---
	let arena2 = Bump::new();
	let rtxn = storage
		.graph_env
		.read_txn()
		.expect("failed to start read txn");

	println!("\n--- All 'person' nodes ---");
	let persons: Vec<_> = G::new(&storage, &rtxn, &arena2)
		.n_from_type(arena2.alloc_str("person"))
		.filter_map(|r| r.ok())
		.collect();

	for tv in &persons {
		if let TraversalValue::Node(n) = tv {
			let json = sonic_rs::to_string_pretty(n).unwrap_or_else(|_| format!("{n}"));
			println!("{json}");
		}
	}

	println!("\n--- Alice's outgoing 'knows' neighbors ---");
	let alice_knows: Vec<_> = G::new(&storage, &rtxn, &arena2)
		.n_from_id(&alice_id)
		.out_node(arena2.alloc_str("knows"))
		.filter_map(|r| r.ok())
		.collect();

	for tv in &alice_knows {
		if let TraversalValue::Node(n) = tv {
			let json = sonic_rs::to_string_pretty(n).unwrap_or_else(|_| format!("{n}"));
			println!("{json}");
		}
	}

	println!("\n--- Who uses Rust? (incoming 'uses' on Rust node) ---");
	let rust_users: Vec<_> = G::new(&storage, &rtxn, &arena2)
		.n_from_id(&rust_id)
		.in_node(arena2.alloc_str("uses"))
		.filter_map(|r| r.ok())
		.collect();

	for tv in &rust_users {
		if let TraversalValue::Node(n) = tv {
			let json = sonic_rs::to_string_pretty(n).unwrap_or_else(|_| format!("{n}"));
			println!("{json}");
		}
	}

	println!("\nDone. Database persisted at: {db_path}");
}
