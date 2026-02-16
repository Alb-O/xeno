use crate::Value;

#[derive(Debug)]
pub struct Example<'a> {
	pub example: &'a str,
	pub description: &'a str,
	pub result: Option<Value>,
}
