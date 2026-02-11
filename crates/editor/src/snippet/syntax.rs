#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetTemplate {
	pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
	Text(String),
	Field(Field),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
	pub index: u32,
	pub kind: FieldKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
	Tabstop,
	Placeholder(Vec<Node>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnippetParseError {
	ExpectedTabstop { pos: usize },
	InvalidFieldBody { pos: usize },
	ExpectedClosingBrace { pos: usize },
}

pub fn parse_snippet_template(input: &str) -> Result<SnippetTemplate, SnippetParseError> {
	let mut parser = Parser::new(input);
	let nodes = parser.parse_nodes(false)?;
	Ok(SnippetTemplate { nodes })
}

struct Parser {
	chars: Vec<char>,
	pos: usize,
}

impl Parser {
	fn new(input: &str) -> Self {
		Self {
			chars: input.chars().collect(),
			pos: 0,
		}
	}

	fn peek(&self) -> Option<char> {
		self.chars.get(self.pos).copied()
	}

	fn next(&mut self) -> Option<char> {
		let ch = self.peek()?;
		self.pos = self.pos.saturating_add(1);
		Some(ch)
	}

	fn parse_nodes(&mut self, stop_on_rbrace: bool) -> Result<Vec<Node>, SnippetParseError> {
		let mut nodes = Vec::new();
		let mut text = String::new();

		while let Some(ch) = self.peek() {
			if stop_on_rbrace && ch == '}' {
				break;
			}

			match ch {
				'\\' => {
					self.next();
					if let Some(escaped) = self.next() {
						text.push(escaped);
					} else {
						text.push('\\');
					}
				}
				'$' => {
					self.next();
					match self.peek() {
						Some('$') => {
							self.next();
							text.push('$');
						}
						Some(next) if next.is_ascii_digit() => {
							flush_text(&mut nodes, &mut text);
							let index = self.parse_index()?;
							nodes.push(Node::Field(Field {
								index,
								kind: FieldKind::Tabstop,
							}));
						}
						Some('{') => {
							self.next();
							flush_text(&mut nodes, &mut text);
							let field = self.parse_braced_field()?;
							nodes.push(Node::Field(field));
						}
						_ => {
							text.push('$');
						}
					}
				}
				_ => {
					self.next();
					text.push(ch);
				}
			}
		}

		flush_text(&mut nodes, &mut text);
		Ok(nodes)
	}

	fn parse_index(&mut self) -> Result<u32, SnippetParseError> {
		let start = self.pos;
		let mut digits = String::new();
		while let Some(ch) = self.peek()
			&& ch.is_ascii_digit()
		{
			digits.push(ch);
			self.next();
		}

		if digits.is_empty() {
			return Err(SnippetParseError::ExpectedTabstop { pos: start });
		}

		digits.parse().map_err(|_| SnippetParseError::ExpectedTabstop { pos: start })
	}

	fn parse_braced_field(&mut self) -> Result<Field, SnippetParseError> {
		let index = self.parse_index()?;
		match self.peek() {
			Some('}') => {
				self.next();
				Ok(Field {
					index,
					kind: FieldKind::Tabstop,
				})
			}
			Some(':') => {
				self.next();
				let placeholder = self.parse_nodes(true)?;
				if self.peek() != Some('}') {
					return Err(SnippetParseError::ExpectedClosingBrace { pos: self.pos });
				}
				self.next();
				Ok(Field {
					index,
					kind: FieldKind::Placeholder(placeholder),
				})
			}
			_ => Err(SnippetParseError::InvalidFieldBody { pos: self.pos }),
		}
	}
}

fn flush_text(nodes: &mut Vec<Node>, text: &mut String) {
	if text.is_empty() {
		return;
	}
	nodes.push(Node::Text(std::mem::take(text)));
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_nested_placeholder_structure() {
		let template = parse_snippet_template("${1:foo ${2:bar}} baz").unwrap();
		assert_eq!(template.nodes.len(), 2);

		let Node::Field(outer) = &template.nodes[0] else {
			panic!("expected first node to be a field");
		};
		assert_eq!(outer.index, 1);

		let FieldKind::Placeholder(children) = &outer.kind else {
			panic!("expected placeholder children");
		};
		assert_eq!(children.len(), 2);
		assert_eq!(children[0], Node::Text("foo ".to_string()));

		let Node::Field(inner) = &children[1] else {
			panic!("expected nested field");
		};
		assert_eq!(inner.index, 2);
		let FieldKind::Placeholder(inner_children) = &inner.kind else {
			panic!("expected nested placeholder children");
		};
		assert_eq!(inner_children, &vec![Node::Text("bar".to_string())]);

		assert_eq!(template.nodes[1], Node::Text(" baz".to_string()));
	}

	#[test]
	fn parses_braced_tabstop() {
		let template = parse_snippet_template("before ${7} after").unwrap();
		assert_eq!(template.nodes.len(), 3);

		let Node::Field(field) = &template.nodes[1] else {
			panic!("expected middle node field");
		};
		assert_eq!(field.index, 7);
		assert_eq!(field.kind, FieldKind::Tabstop);
	}
}
