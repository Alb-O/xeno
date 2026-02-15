use super::*;

#[test]
fn deserialize_basic_record() {
	let value = nuon::from_nuon(r#"{ name: "test", count: 42, enabled: true }"#, None).unwrap();

	#[derive(serde::Deserialize, Debug, PartialEq)]
	struct Basic {
		name: String,
		count: i64,
		enabled: bool,
	}

	let result: Basic = from_nu_value(&value).unwrap();
	assert_eq!(
		result,
		Basic {
			name: "test".into(),
			count: 42,
			enabled: true
		}
	);
}

#[test]
fn deserialize_externally_tagged_enum() {
	let value = nuon::from_nuon(r#"{ Git: { remote: "https://example.com", revision: "main" } }"#, None).unwrap();

	#[derive(serde::Deserialize, Debug, PartialEq)]
	enum Source {
		Git { remote: String, revision: String },
		Local { path: String },
	}

	let result: Source = from_nu_value(&value).unwrap();
	assert_eq!(
		result,
		Source::Git {
			remote: "https://example.com".into(),
			revision: "main".into()
		}
	);
}

#[test]
fn deserialize_option_and_tuple() {
	let value = nuon::from_nuon(r#"{ comment: ["/*", "*/"], nothing: null }"#, None).unwrap();

	#[derive(serde::Deserialize, Debug, PartialEq)]
	struct WithOpt {
		comment: Option<(String, String)>,
		nothing: Option<String>,
	}

	let result: WithOpt = from_nu_value(&value).unwrap();
	assert_eq!(
		result,
		WithOpt {
			comment: Some(("/*".into(), "*/".into())),
			nothing: None,
		}
	);
}

#[test]
fn deserialize_list_and_defaults() {
	let value = nuon::from_nuon(r#"{ items: [1, 2, 3] }"#, None).unwrap();

	#[derive(serde::Deserialize, Debug, PartialEq)]
	struct WithList {
		items: Vec<i64>,
		#[serde(default)]
		missing: Vec<String>,
	}

	let result: WithList = from_nu_value(&value).unwrap();
	assert_eq!(
		result,
		WithList {
			items: vec![1, 2, 3],
			missing: vec![]
		}
	);
}

#[test]
fn deserialize_hashmap() {
	let value = nuon::from_nuon(r##"{ fg: "#ff0000", bg: "#000000" }"##, None).unwrap();

	let result: std::collections::HashMap<String, String> = from_nu_value(&value).unwrap();
	assert_eq!(result.len(), 2);
	assert_eq!(result["fg"], "#ff0000");
}
