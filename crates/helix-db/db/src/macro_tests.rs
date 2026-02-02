#[cfg(test)]
mod macro_regressions {
	use helix_macros::helix_node;
	use serde::{Deserialize, Serialize};

	#[helix_node]
	#[derive(Debug, Serialize, Deserialize)]
	pub struct TestNode {
		pub name: String,
	}

	#[test]
	fn test_node_id_field_exists() {
		let node = TestNode {
			id: "test".to_string(),
			name: "test".to_string(),
		};
		assert_eq!(node.id, "test");
	}
}

// Ensure 'helix_db' name is available for macros when testing internally
#[cfg(test)]
extern crate self as helix_db;
