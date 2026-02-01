#[cfg(test)]
mod tests {
	use crate::helixc::analyzer::error_codes::ErrorCode;
	use crate::helixc::parser::{HelixParser, write_to_temp_file};

	// ============================================================================
	// AddNode Expression Tests
	// ============================================================================

	#[test]
	fn test_add_node_valid() {
		let source = r#"
            N::Person { name: String, age: U32 }

            QUERY test(personName: String, personAge: U32) =>
                person <- AddN<Person>({name: personName, age: personAge})
                RETURN person
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	#[test]
	fn test_add_node_undeclared_type() {
		let source = r#"
            N::Person { name: String }

            QUERY test() =>
                company <- AddN<Company>({name: "Acme"})
                RETURN company
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.iter().any(|d| d.error_code == ErrorCode::E101));
	}

	// ============================================================================
	// AddEdge Expression Tests
	// ============================================================================

	#[test]
	fn test_add_edge_valid() {
		let source = r#"
            N::Person { name: String }
            E::Knows { From: Person, To: Person }

            QUERY test(id1: ID, id2: ID) =>
                person1 <- N<Person>(id1)
                person2 <- N<Person>(id2)
                edge <- AddE<Knows>::From(person1)::To(person2)
                RETURN edge
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	#[test]
	fn test_add_edge_with_unique_index_valid() {
		let source = r#"
                N::Person { name: String }
                E::Knows UNIQUE { From: Person, To: Person }

                QUERY test(id1: ID, id2: ID) =>
                    person1 <- N<Person>(id1)
                    person2 <- N<Person>(id2)
                    edge <- AddE<Knows>::From(person1)::To(person2)
                    RETURN edge
            "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.is_empty());
	}

	#[test]
	fn test_add_edge_undeclared_type() {
		let source = r#"
            N::Person { name: String }

            QUERY test(id1: ID, id2: ID) =>
                edge <- AddE<UndeclaredEdge>::From(id1)::To(id2)
                RETURN edge
        "#;

		let content = write_to_temp_file(vec![source]);
		let parsed = HelixParser::parse_source(&content).unwrap();
		let result = crate::helixc::analyzer::analyze(&parsed);

		assert!(result.is_ok());
		let (diagnostics, _) = result.unwrap();
		assert!(diagnostics.iter().any(|d| d.error_code == ErrorCode::E102));
	}

	// ============================================================================
	// Array Literal Tests
	// ============================================================================

	#[test]
	fn test_array_literal_homogeneous() {
		let source = r#"
            N::Person { name: String }

            QUERY test() =>
                names <- ["Alice", "Bob", "Charlie"]
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
