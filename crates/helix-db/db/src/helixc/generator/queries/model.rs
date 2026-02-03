use super::Parameter;
use crate::helixc::generator::return_values::{ReturnValue, ReturnValueStruct};
use crate::helixc::generator::statements::Statement;
use crate::helixc::generator::utils::EmbedData;

pub struct Query {
	pub embedding_model_to_use: Option<String>,
	pub mcp_handler: Option<String>,
	pub name: String,
	pub statements: Vec<Statement>,
	pub parameters: Vec<Parameter>,
	pub sub_parameters: Vec<(String, Vec<Parameter>)>,
	pub return_values: Vec<(String, ReturnValue)>,
	pub return_structs: Vec<ReturnValueStruct>,
	pub use_struct_returns: bool,
	pub is_mut: bool,
	pub hoisted_embedding_calls: Vec<EmbedData>,
}

impl Default for Query {
	fn default() -> Self {
		Self {
			embedding_model_to_use: None,
			mcp_handler: None,
			name: "".to_string(),
			statements: vec![],
			parameters: vec![],
			sub_parameters: vec![],
			return_values: vec![],
			return_structs: vec![],
			use_struct_returns: true,
			is_mut: false,
			hoisted_embedding_calls: vec![],
		}
	}
}

impl Query {
	pub fn add_hoisted_embed(&mut self, embed_data: EmbedData) -> String {
		let name = EmbedData::name_from_index(self.hoisted_embedding_calls.len());
		self.hoisted_embedding_calls.push(embed_data);
		name
	}
}
