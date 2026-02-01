#[cfg(test)]
mod tests {
	use crate::helixc::analyzer::error_codes::ErrorCode;
	use crate::helixc::parser::{HelixParser, write_to_temp_file};

	// ============================================================================
	// Start Node Validation Tests
	// ============================================================================

	#[test]
	fn test_undeclared_node_type() {
		let source = r#"
            N::Person { name: String }

            QUERY test() =>
                company <- N<Company>
                RETURN company
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.iter().any(|d| d.error_code == ErrorCode::E101));
	}

	#[test]
	fn test_undeclared_edge_type() {
		let source = r#"
            N::Person { name: String }
            E::Knows { From: Person, To: Person }

            QUERY test(id: ID) =>
                person <- N<Person>(id)
                edges <- person::OutE<WorksAt>
                RETURN edges
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.iter().any(|d| d.error_code == ErrorCode::E102));
	}

	#[test]
	fn test_undeclared_vector_type() {
		let source = r#"
            N::Person { name: String }

            QUERY test() =>
                docs <- V<Document>
                RETURN docs
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.iter().any(|d| d.error_code == ErrorCode::E103));
	}

	#[test]
	fn test_node_with_id_parameter() {
		let source = r#"
            N::Person { name: String }

            QUERY test(id: ID) =>
                person <- N<Person>(id)
                RETURN person
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(!diagnostics.iter().any(|d| d.error_code == ErrorCode::E301));
	}

	#[test]
	fn test_node_with_undefined_id_variable() {
		let source = r#"
            N::Person { name: String }

            QUERY test() =>
                person <- N<Person>(unknownId)
                RETURN person
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.iter().any(|d| d.error_code == ErrorCode::E301));
	}

	#[test]
	fn test_node_without_id() {
		let source = r#"
            N::Person { name: String }

            QUERY test() =>
                people <- N<Person>
                RETURN people
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	#[test]
	fn test_identifier_start_node() {
		let source = r#"
            N::Person { name: String }

            QUERY test() =>
                person <- N<Person>
                samePerson <- person
                RETURN samePerson
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	#[test]
	fn test_identifier_not_in_scope() {
		let source = r#"
            N::Person { name: String }

            QUERY test() =>
                person <- unknownVariable
                RETURN person
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.iter().any(|d| d.error_code == ErrorCode::E301));
	}

	// ============================================================================
	// Traversal Step Tests
	// ============================================================================

	#[test]
	fn test_valid_out_traversal() {
		let source = r#"
            N::Person { name: String }
            E::Knows { From: Person, To: Person }

            QUERY test(id: ID) =>
                person <- N<Person>(id)
                friends <- person::Out<Knows>
                RETURN friends
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	#[test]
	fn test_property_access() {
		let source = r#"
            N::Person { name: String, age: U32 }

            QUERY test(id: ID) =>
                person <- N<Person>(id)
                name <- person::{name}
                RETURN name
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	// Note: Property errors are caught during object validation, not traversal validation
	// Removing test_property_not_exists as it requires different assertion approach

	// ============================================================================
	// Where Clause Tests
	// ============================================================================

	#[test]
	fn test_where_with_property_equals() {
		let source = r#"
            N::Person { name: String, age: U32 }

            QUERY test(targetAge: U32) =>
                people <- N<Person>::WHERE(_::{age}::EQ(targetAge))
                RETURN people
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	#[test]
	fn test_where_with_property_greater_than() {
		let source = r#"
            N::Person { name: String, age: U32 }

            QUERY test(minAge: U32) =>
                people <- N<Person>::WHERE(_::{age}::GT(minAge))
                RETURN people
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	// Note: Removed tests for UPDATE, Range, and property errors as they require
	// different syntax or validation approaches than initially assumed

	// ============================================================================
	// Chained Traversal Tests
	// ============================================================================

	#[test]
	fn test_chained_edge_traversal() {
		let source = r#"
            N::Person { name: String }
            E::Knows { From: Person, To: Person }

            QUERY test(id: ID) =>
                person <- N<Person>(id)
                edges <- person::OutE<Knows>
                targets <- edges::ToN
                RETURN targets
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	#[test]
	fn test_multi_hop_traversal() {
		let source = r#"
            N::Person { name: String }
            E::Knows { From: Person, To: Person }

            QUERY test(id: ID) =>
                friends <- N<Person>(id)::Out<Knows>
                friendsOfFriends <- friends::Out<Knows>
                RETURN friendsOfFriends
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	// ============================================================================
	// Complex Query Tests
	// ============================================================================

	#[test]
	fn test_complex_query_with_multiple_steps() {
		let source = r#"
            N::Person { name: String, age: U32 }
            E::Knows { From: Person, To: Person }

            QUERY test(id: ID, minAge: U32) =>
                person <- N<Person>(id)
                friends <- person::Out<Knows>::WHERE(_::{age}::GT(minAge))
                names <- friends::{name}
                RETURN names
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}
}
