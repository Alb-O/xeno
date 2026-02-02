#[cfg(test)]
mod macro_regressions {
    use helix_macros::{helix_node, tool_call};
    use serde::{Deserialize, Serialize};
    use crate::helix_engine::types::EngineError;
    use crate::protocol::Response;
    use crate::helix_engine::traversal_core::traversal_value::TraversalValue;
    use crate::protocol::format::Format;

    #[helix_node]
    #[derive(Debug, Serialize, Deserialize)]
    pub struct TestNode {
        pub name: String,
    }

    #[derive(Serialize, Deserialize, Clone)]
    #[allow(non_camel_case_types)]
    pub struct test_tool_readInput {}

    #[tool_call(results, with_read)]
    #[allow(non_snake_case)]
    pub fn test_tool_read(_input: crate::helix_gateway::router::router::HandlerInput) -> Result<Response, EngineError> {
        {
            let results = vec![TraversalValue::Empty];
            let _ = results;
        }
        Ok(Response { body: vec![], fmt: Format::Json })
    }

    #[derive(Serialize, Deserialize, Clone)]
    #[allow(non_camel_case_types)]
    pub struct test_tool_writeInput {}

    #[tool_call(results, with_write)]
    #[allow(non_snake_case)]
    pub fn test_tool_write(_input: crate::helix_gateway::router::router::HandlerInput) -> Result<Response, EngineError> {
        {
            let results = vec![TraversalValue::Empty];
            let _ = results;
        }
        Ok(Response { body: vec![], fmt: Format::Json })
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
