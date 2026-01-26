use super::*;
use crate::{RegistryMeta, RegistrySource};

/// Test definition type.
#[derive(Debug, PartialEq, Eq)]
struct TestDef {
	meta: RegistryMeta,
}

impl RegistryEntry for TestDef {
	fn meta(&self) -> &RegistryMeta {
		&self.meta
	}
}

static DEF_A: TestDef = TestDef {
	meta: RegistryMeta {
		id: "test::a",
		name: "a",
		aliases: &["alpha"],
		description: "Test A",
		priority: 0,
		source: RegistrySource::Builtin,
		required_caps: &[],
		flags: 0,
	},
};

static DEF_B: TestDef = TestDef {
	meta: RegistryMeta {
		id: "test::b",
		name: "b",
		aliases: &[],
		description: "Test B",
		priority: 10,
		source: RegistrySource::Builtin,
		required_caps: &[],
		flags: 0,
	},
};

#[test]
fn test_index_lookup() {
	let index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_B)
		.duplicate_policy(DuplicatePolicy::Panic)
		.build();

	assert_eq!(index.len(), 2);

	// Lookup by name
	assert!(std::ptr::eq(index.get("a").unwrap(), &DEF_A));
	assert!(std::ptr::eq(index.get("b").unwrap(), &DEF_B));

	// Lookup by id
	assert!(std::ptr::eq(index.get("test::a").unwrap(), &DEF_A));

	// Lookup by alias
	assert!(std::ptr::eq(index.get("alpha").unwrap(), &DEF_A));

	// Not found
	assert!(index.get("unknown").is_none());
}

#[test]
fn test_sort_default() {
	let index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_B)
		.sort_default()
		.build();

	// DEF_B has higher priority (10), so it comes first.
	assert!(std::ptr::eq(index.items()[0], &DEF_B));
	assert!(std::ptr::eq(index.items()[1], &DEF_A));
}

#[test]
fn test_first_wins() {
	static DEF_A2: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::a2",
			name: "a", // Same name as DEF_A
			aliases: &[],
			description: "Test A2",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	let index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_A2)
		.duplicate_policy(DuplicatePolicy::FirstWins)
		.build();

	// First wins: DEF_A should be in the index for key "a".
	assert!(std::ptr::eq(index.get("a").unwrap(), &DEF_A));
	// But DEF_A2 is still in items.
	assert_eq!(index.len(), 2);
}

#[test]
fn test_last_wins() {
	static DEF_A2: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::a2",
			name: "a",
			aliases: &[],
			description: "Test A2",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	let index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_A2)
		.duplicate_policy(DuplicatePolicy::LastWins)
		.build();

	// Last wins: DEF_A2 should be in the index for key "a".
	assert!(std::ptr::eq(index.get("a").unwrap(), &DEF_A2));
}

#[test]
#[should_panic(expected = "duplicate registry key")]
fn test_panic_on_duplicate() {
	static DEF_A2: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::a2",
			name: "a",
			aliases: &[],
			description: "Test A2",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	let _index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_A2)
		.duplicate_policy(DuplicatePolicy::Panic)
		.build();
}

#[test]
#[should_panic(expected = "duplicate ID")]
fn test_duplicate_id_fatal_regardless_of_policy() {
	// Two definitions with same ID should always be fatal, even with FirstWins
	static DEF_DUP_ID: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::a", // Same ID as DEF_A
			name: "different_name",
			aliases: &[],
			description: "Duplicate ID",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	let _index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_DUP_ID)
		.duplicate_policy(DuplicatePolicy::FirstWins) // Not Panic!
		.build();
}

#[test]
#[should_panic(expected = "shadows ID")]
fn test_name_shadows_id_fatal() {
	// Name that equals another definition's ID should be fatal
	static DEF_NAME_SHADOWS: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::shadow",
			name: "test::a", // Name equals DEF_A's ID
			aliases: &[],
			description: "Name shadows ID",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	let _index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_NAME_SHADOWS)
		.duplicate_policy(DuplicatePolicy::FirstWins) // Not Panic!
		.build();
}

#[test]
#[should_panic(expected = "shadows ID")]
fn test_alias_shadows_id_fatal() {
	// Alias that equals another definition's ID should be fatal
	static DEF_ALIAS_SHADOWS: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::shadow2",
			name: "shadow2",
			aliases: &["test::a"], // Alias equals DEF_A's ID
			description: "Alias shadows ID",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	let _index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_ALIAS_SHADOWS)
		.duplicate_policy(DuplicatePolicy::FirstWins) // Not Panic!
		.build();
}

#[test]
fn test_collision_recorded() {
	// Name collision should be recorded (not fatal with FirstWins)
	static DEF_COLLISION: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::collision",
			name: "a", // Same name as DEF_A
			aliases: &[],
			description: "Collision test",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	let index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_COLLISION)
		.duplicate_policy(DuplicatePolicy::FirstWins)
		.build();

	// Should have recorded the collision
	assert_eq!(index.collisions().len(), 1);
	let collision = &index.collisions()[0];
	assert_eq!(collision.kind, KeyKind::Name);
	assert_eq!(collision.key, "a");
	assert_eq!(collision.existing_id, "test::a");
	assert_eq!(collision.new_id, "test::collision");
	assert_eq!(collision.winner_id, "test::a"); // FirstWins
}

#[test]
fn test_id_first_lookup() {
	// ID lookup should take precedence over name/alias
	static DEF_ID_FIRST: TestDef = TestDef {
		meta: RegistryMeta {
			id: "lookup_target", // This is the ID
			name: "different_name",
			aliases: &[],
			description: "ID first test",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	let index = RegistryBuilder::new("test").push(&DEF_ID_FIRST).build();

	// Lookup by ID should work
	assert!(std::ptr::eq(
		index.get("lookup_target").unwrap(),
		&DEF_ID_FIRST
	));
	assert!(std::ptr::eq(
		index.get_by_id("lookup_target").unwrap(),
		&DEF_ID_FIRST
	));
}

#[test]
fn test_items_all_vs_effective() {
	// items_all includes shadowed, items (effective) excludes shadowed
	static DEF_SHADOWED: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::shadowed",
			name: "a", // Same name as DEF_A
			aliases: &[],
			description: "Shadowed def",
			priority: -1, // Lower priority
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	};

	let index = RegistryBuilder::new("test")
		.push(&DEF_A)
		.push(&DEF_SHADOWED)
		.sort_default() // DEF_A (priority 0) before DEF_SHADOWED (priority -1)
		.duplicate_policy(DuplicatePolicy::FirstWins)
		.build();

	// items_all contains both
	assert_eq!(index.items_all().len(), 2);

	// items (effective) contains both because both have unique IDs
	// and are therefore reachable via by_id
	assert_eq!(index.items().len(), 2);

	// But lookup by name "a" returns DEF_A (first wins)
	assert!(std::ptr::eq(index.get("a").unwrap(), &DEF_A));
}

#[test]
#[should_panic(expected = "duplicate ID")]
fn test_runtime_duplicate_id_with_builtin_fatal() {
	// Runtime def with same ID as builtin should be fatal
	static DEF_RUNTIME_DUP: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::a", // Same ID as DEF_A
			name: "runtime_name",
			aliases: &[],
			description: "Runtime duplicate ID",
			priority: 0,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	};

	let builtins = RegistryBuilder::new("test")
		.push(&DEF_A)
		.duplicate_policy(DuplicatePolicy::FirstWins)
		.build();

	let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
	registry.register(&DEF_RUNTIME_DUP);
}

#[test]
#[should_panic(expected = "shadows ID")]
fn test_runtime_name_shadows_builtin_id_fatal() {
	// Runtime name that equals builtin ID should be fatal
	static DEF_RUNTIME_SHADOW: TestDef = TestDef {
		meta: RegistryMeta {
			id: "test::runtime_shadow",
			name: "test::a", // Name equals builtin ID
			aliases: &[],
			description: "Runtime name shadows builtin ID",
			priority: 0,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	};

	let builtins = RegistryBuilder::new("test")
		.push(&DEF_A)
		.duplicate_policy(DuplicatePolicy::FirstWins)
		.build();

	let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
	registry.register(&DEF_RUNTIME_SHADOW);
}

#[test]
fn test_runtime_id_first_lookup() {
	// Runtime registry should use ID-first lookup
	static DEF_RUNTIME: TestDef = TestDef {
		meta: RegistryMeta {
			id: "runtime::def",
			name: "runtime_name",
			aliases: &[],
			description: "Runtime def",
			priority: 0,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	};

	let builtins = RegistryBuilder::new("test")
		.push(&DEF_A)
		.duplicate_policy(DuplicatePolicy::FirstWins)
		.build();

	let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
	registry.register(&DEF_RUNTIME);

	// Lookup by ID should work for both builtin and runtime
	assert!(std::ptr::eq(registry.get("test::a").unwrap(), &DEF_A));
	assert!(std::ptr::eq(
		registry.get("runtime::def").unwrap(),
		&DEF_RUNTIME
	));

	// get_by_id should also work
	assert!(std::ptr::eq(registry.get_by_id("test::a").unwrap(), &DEF_A));
	assert!(std::ptr::eq(
		registry.get_by_id("runtime::def").unwrap(),
		&DEF_RUNTIME
	));
}

#[test]
#[should_panic(expected = "shadows ID")]
fn test_runtime_name_shadows_runtime_id_fatal() {
	static DEF_RUNTIME1: TestDef = TestDef {
		meta: RegistryMeta {
			id: "runtime::first",
			name: "first_name",
			aliases: &[],
			description: "Runtime def 1",
			priority: 0,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	};

	static DEF_RUNTIME2: TestDef = TestDef {
		meta: RegistryMeta {
			id: "runtime::second",
			name: "runtime::first", // Name equals first runtime def's ID
			aliases: &[],
			description: "Runtime def 2",
			priority: 0,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	};

	let builtins = RegistryBuilder::new("test")
		.duplicate_policy(DuplicatePolicy::FirstWins)
		.build();

	let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
	registry.register(&DEF_RUNTIME1);
	registry.register(&DEF_RUNTIME2);
}

#[test]
fn test_runtime_collision_recorded() {
	static DEF_RUNTIME1: TestDef = TestDef {
		meta: RegistryMeta {
			id: "runtime::first",
			name: "shared_name",
			aliases: &[],
			description: "Runtime def 1",
			priority: 0,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	};

	static DEF_RUNTIME2: TestDef = TestDef {
		meta: RegistryMeta {
			id: "runtime::second",
			name: "shared_name", // Same name, should record collision
			aliases: &[],
			description: "Runtime def 2",
			priority: 0,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
	};

	let builtins = RegistryBuilder::new("test")
		.duplicate_policy(DuplicatePolicy::FirstWins)
		.build();

	let registry = RuntimeRegistry::with_policy("test", builtins, DuplicatePolicy::FirstWins);
	registry.register(&DEF_RUNTIME1);
	registry.register(&DEF_RUNTIME2);

	let collisions = registry.collisions();
	assert_eq!(collisions.len(), 1);
	assert_eq!(collisions[0].kind, KeyKind::Name);
	assert_eq!(collisions[0].key, "shared_name");
	assert_eq!(collisions[0].existing_id, "runtime::first");
	assert_eq!(collisions[0].new_id, "runtime::second");
	assert_eq!(collisions[0].winner_id, "runtime::first"); // FirstWins
}

fn meta(id: &'static str, name: &'static str, priority: i16) -> RegistryMeta {
	RegistryMeta {
		id,
		name,
		aliases: &[],
		description: "",
		priority,
		source: RegistrySource::Builtin,
		required_caps: &[],
		flags: 0,
	}
}

fn meta_with(
	id: &'static str,
	name: &'static str,
	priority: i16,
	source: RegistrySource,
) -> RegistryMeta {
	RegistryMeta {
		id,
		name,
		aliases: &[],
		description: "",
		priority,
		source,
		required_caps: &[],
		flags: 0,
	}
}

fn leak_def(meta: RegistryMeta) -> &'static TestDef {
	Box::leak(Box::new(TestDef { meta }))
}

#[test]
fn runtime_register_shadowing_is_atomic() {
	let builtin = leak_def(RegistryMeta {
		id: "builtin.id",
		name: "builtin.name",
		aliases: &[],
		description: "",
		priority: 0,
		source: RegistrySource::Builtin,
		required_caps: &[],
		flags: 0,
	});

	let builtins = RegistryBuilder::new("t")
		.include_aliases(false)
		.push(builtin)
		.build();

	let rr = RuntimeRegistry::new("t", builtins);

	// This def's *name* shadows an existing ID => fatal.
	let bad = leak_def(RegistryMeta {
		id: "runtime.id",
		name: "builtin.id", // <-- shadows ID
		aliases: &[],
		description: "",
		priority: 0,
		source: RegistrySource::Runtime,
		required_caps: &[],
		flags: 0,
	});

	let before_all = rr.all();
	let before_collisions = rr.collisions();

	let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		rr.register(bad);
	}));
	assert!(res.is_err());

	// No ghost insert by id/name
	assert!(rr.get_by_id("runtime.id").is_none());
	assert!(rr.get("runtime.id").is_none());
	assert_eq!(rr.all(), before_all);
	assert_eq!(rr.collisions(), before_collisions);
}

#[test]
fn runtime_duplicate_id_does_not_overwrite() {
	let builtins = RegistryBuilder::new("t").build();
	let rr = RuntimeRegistry::new("t", builtins);

	let a = leak_def(meta("dup.id", "a", 0));
	let b = leak_def(meta("dup.id", "b", 0));

	assert!(rr.register(a));
	assert!(rr.get("dup.id").is_some());
	assert!(std::ptr::eq(rr.get("dup.id").unwrap(), a));

	let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		rr.register(b);
	}));
	assert!(res.is_err());

	// Still A
	assert!(std::ptr::eq(rr.get("dup.id").unwrap(), a));
}

#[test]
fn runtime_bypriority_winner_is_order_independent() {
	let builtins = RegistryBuilder::new("t").build();

	let hi = leak_def(meta_with("id.hi", "same", 100, RegistrySource::Crate("x")));
	let lo = leak_def(meta_with("id.lo", "same", 0, RegistrySource::Runtime));

	// order 1
	let rr1 = RuntimeRegistry::with_policy("t", builtins.clone(), DuplicatePolicy::ByPriority);
	rr1.register(lo);
	rr1.register(hi);
	assert!(std::ptr::eq(rr1.get("same").unwrap(), hi));

	// order 2
	let rr2 = RuntimeRegistry::with_policy("t", builtins, DuplicatePolicy::ByPriority);
	rr2.register(hi);
	rr2.register(lo);
	assert!(std::ptr::eq(rr2.get("same").unwrap(), hi));
}

#[test]
fn test_total_order_tie_breaker() {
	let builtins = RegistryBuilder::new("t").build();
	let rr = RuntimeRegistry::with_policy("t", builtins, DuplicatePolicy::ByPriority);

	// 1. Priority (Higher wins)
	let hi_prio = leak_def(meta_with("id.hi", "same", 100, RegistrySource::Runtime));
	let lo_prio = leak_def(meta_with("id.lo", "same", 0, RegistrySource::Runtime));
	rr.register(lo_prio);
	rr.register(hi_prio);
	assert!(std::ptr::eq(rr.get("same").unwrap(), hi_prio));

	// 2. Source Rank (Builtin > Crate > Runtime)
	let builtin_src = leak_def(meta_with("id.builtin", "src", 10, RegistrySource::Builtin));
	let crate_src = leak_def(meta_with("id.crate", "src", 10, RegistrySource::Crate("x")));
	let runtime_src = leak_def(meta_with("id.runtime", "src", 10, RegistrySource::Runtime));

	let rr_src = RuntimeRegistry::with_policy(
		"src",
		RegistryIndex {
			by_id: HashMap::from([("id.builtin", builtin_src)]),
			by_key: HashMap::from([("src", builtin_src)]),
			items_all: vec![builtin_src],
			items_effective: vec![builtin_src],
			collisions: vec![],
		},
		DuplicatePolicy::ByPriority,
	);

	rr_src.register(crate_src);
	rr_src.register(runtime_src);

	// Runtime should win over Crate and Builtin at same priority
	assert!(std::ptr::eq(rr_src.get("src").unwrap(), runtime_src));

	// 3. ID (Lexical higher wins)
	let a_id = leak_def(meta_with("id.a", "lex", 10, RegistrySource::Runtime));
	let b_id = leak_def(meta_with("id.b", "lex", 10, RegistrySource::Runtime));
	let rr_lex = RuntimeRegistry::with_policy(
		"lex",
		RegistryBuilder::new("t").build(),
		DuplicatePolicy::ByPriority,
	);
	rr_lex.register(a_id);
	rr_lex.register(b_id);
	assert!(std::ptr::eq(rr_lex.get("lex").unwrap(), b_id));
}
