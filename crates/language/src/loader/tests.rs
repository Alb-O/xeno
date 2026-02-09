use super::*;

fn test_db() -> Arc<LanguageDb> {
	let mut db = LanguageDb::new();
	db.register(LanguageData::new(
		"rust".to_string(),
		None,
		vec!["rs".to_string()],
		vec![],
		vec![],
		vec![],
		vec!["//".to_string()],
		Some(("/*".to_string(), "*/".to_string())),
		None,
		vec![],
		vec![],
	));
	db.register(LanguageData::new(
		"python".to_string(),
		None,
		vec!["py".to_string()],
		vec![],
		vec![],
		vec!["python".to_string()],
		vec!["#".to_string()],
		None,
		None,
		vec![],
		vec![],
	));
	Arc::new(db)
}

#[test]
fn loader_from_db() {
	let db = test_db();
	let loader = LanguageLoader::from_db(db);

	let lang = loader.language_for_name("rust").unwrap();
	assert_eq!(lang.idx(), 0);
	assert_eq!(loader.language_for_path(Path::new("test.rs")), Some(lang));
}

#[test]
fn shebang_detection() {
	let db = test_db();
	let loader = LanguageLoader::from_db(db);

	let lang = loader.language_for_name("python").unwrap();

	assert_eq!(loader.language_for_shebang("#!/usr/bin/python"), Some(lang));
	assert_eq!(
		loader.language_for_shebang("#!/usr/bin/env python"),
		Some(lang)
	);
	assert_eq!(
		loader.language_for_shebang("#!/usr/bin/python3"),
		Some(lang)
	);
	assert_eq!(loader.language_for_shebang("not a shebang"), None);
}

#[test]
fn from_embedded_uses_global_db() {
	let loader = LanguageLoader::from_embedded();
	assert!(!loader.is_empty());

	let rust = loader.language_for_name("rust").expect("rust language");
	let data = loader.get(rust).unwrap();
	assert!(data.extensions.contains(&"rs".to_string()));
}
