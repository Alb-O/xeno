use super::helpers::*;

#[test]
fn test_bm25_flatten_properties() {
	let arena = Bump::new();

	let props: HashMap<String, Value> = HashMap::from([
		(
			"title".to_string(),
			Value::String("Test Document".to_string()),
		),
		(
			"content".to_string(),
			Value::String("This is content".to_string()),
		),
		("count".to_string(), Value::I32(42)),
	]);

	let props_map = ImmutablePropertiesMap::new(
		props.len(),
		props
			.iter()
			.map(|(k, v)| (arena.alloc_str(k) as &str, v.clone())),
		&arena,
	);

	let flattened = props_map.flatten_bm25();

	// Should contain all keys and values
	assert!(flattened.contains("title"));
	assert!(flattened.contains("Test Document"));
	assert!(flattened.contains("content"));
	assert!(flattened.contains("This is content"));
	assert!(flattened.contains("count"));
	assert!(flattened.contains("42"));
}
