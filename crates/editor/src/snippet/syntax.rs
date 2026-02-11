#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetTemplate {
	pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
	Text(String),
	Field(Field),
	Var(Var),
	Transform(Transform),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Var {
	pub name: String,
	pub default: Option<Vec<Node>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformSource {
	Tabstop(u32),
	Var(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transform {
	pub source: TransformSource,
	pub regex: String,
	pub replace: String,
	pub flags: String,
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
	Choice(Vec<String>),
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
					let checkpoint = self.pos;
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
							let parsed = match self.peek() {
								Some(next) if next.is_ascii_digit() => self.parse_braced_field(),
								Some(next) if is_identifier_start(next) => self.parse_braced_variable(),
								_ => Err(SnippetParseError::InvalidFieldBody { pos: self.pos }),
							};

							match parsed {
								Ok(node) => nodes.push(node),
								Err(_) => {
									self.pos = checkpoint;
									text.push('$');
									text.push_str(&self.consume_raw_braced_literal());
								}
							}
						}
						Some(next) if is_identifier_start(next) => {
							flush_text(&mut nodes, &mut text);
							let name = self.parse_identifier();
							nodes.push(Node::Var(Var { name, default: None }));
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

	fn parse_braced_field(&mut self) -> Result<Node, SnippetParseError> {
		let index = self.parse_index()?;
		match self.peek() {
			Some('}') => {
				self.next();
				Ok(Node::Field(Field {
					index,
					kind: FieldKind::Tabstop,
				}))
			}
			Some(':') => {
				self.next();
				let placeholder = self.parse_nodes(true)?;
				if self.peek() != Some('}') {
					return Err(SnippetParseError::ExpectedClosingBrace { pos: self.pos });
				}
				self.next();
				Ok(Node::Field(Field {
					index,
					kind: FieldKind::Placeholder(placeholder),
				}))
			}
			Some('|') => {
				self.next();
				let options = self.parse_choice_items()?;
				Ok(Node::Field(Field {
					index,
					kind: FieldKind::Choice(options),
				}))
			}
			Some('/') => {
				self.next();
				Ok(Node::Transform(self.parse_transform(TransformSource::Tabstop(index))?))
			}
			_ => Err(SnippetParseError::InvalidFieldBody { pos: self.pos }),
		}
	}

	fn parse_choice_items(&mut self) -> Result<Vec<String>, SnippetParseError> {
		let mut options: Vec<String> = Vec::new();
		let mut current = String::new();

		while let Some(ch) = self.next() {
			match ch {
				'\\' => {
					if let Some(escaped) = self.next() {
						current.push(escaped);
					} else {
						current.push('\\');
					}
				}
				',' => {
					options.push(std::mem::take(&mut current));
				}
				'|' => {
					if self.peek() != Some('}') {
						return Err(SnippetParseError::ExpectedClosingBrace { pos: self.pos });
					}
					self.next();
					options.push(current);
					return Ok(options);
				}
				_ => {
					current.push(ch);
				}
			}
		}

		Err(SnippetParseError::ExpectedClosingBrace { pos: self.pos })
	}

	fn parse_identifier(&mut self) -> String {
		let mut name = String::new();
		while let Some(ch) = self.peek()
			&& is_identifier_continue(ch)
		{
			name.push(ch);
			self.next();
		}
		name
	}

	fn parse_braced_variable(&mut self) -> Result<Node, SnippetParseError> {
		let name = self.parse_identifier();
		if name.is_empty() {
			return Err(SnippetParseError::InvalidFieldBody { pos: self.pos });
		}

		match self.peek() {
			Some('}') => {
				self.next();
				Ok(Node::Var(Var { name, default: None }))
			}
			Some(':') => {
				self.next();
				let default = self.parse_nodes(true)?;
				if self.peek() != Some('}') {
					return Err(SnippetParseError::ExpectedClosingBrace { pos: self.pos });
				}
				self.next();
				Ok(Node::Var(Var { name, default: Some(default) }))
			}
			Some('/') => {
				self.next();
				Ok(Node::Transform(self.parse_transform(TransformSource::Var(name))?))
			}
			_ => Err(SnippetParseError::InvalidFieldBody { pos: self.pos }),
		}
	}

	fn parse_transform(&mut self, source: TransformSource) -> Result<Transform, SnippetParseError> {
		let regex = self.parse_transform_segment()?;
		let replace = self.parse_transform_segment()?;
		let flags = self.parse_transform_flags()?;
		Ok(Transform { source, regex, replace, flags })
	}

	fn parse_transform_segment(&mut self) -> Result<String, SnippetParseError> {
		let mut out = String::new();
		while let Some(ch) = self.next() {
			match ch {
				'\\' => {
					if let Some(escaped) = self.next() {
						if escaped == '/' || escaped == '\\' {
							out.push(escaped);
						} else {
							out.push('\\');
							out.push(escaped);
						}
					} else {
						out.push('\\');
					}
				}
				'/' => return Ok(out),
				_ => out.push(ch),
			}
		}

		Err(SnippetParseError::ExpectedClosingBrace { pos: self.pos })
	}

	fn parse_transform_flags(&mut self) -> Result<String, SnippetParseError> {
		let mut flags = String::new();
		while let Some(ch) = self.peek() {
			if ch == '}' {
				self.next();
				return Ok(flags);
			}
			flags.push(ch);
			self.next();
		}

		Err(SnippetParseError::ExpectedClosingBrace { pos: self.pos })
	}

	fn consume_raw_braced_literal(&mut self) -> String {
		let mut raw = String::new();
		let mut escaped = false;

		while let Some(ch) = self.next() {
			raw.push(ch);
			if escaped {
				escaped = false;
				continue;
			}
			if ch == '\\' {
				escaped = true;
				continue;
			}
			if ch == '}' {
				break;
			}
		}

		raw
	}
}

fn is_identifier_start(ch: char) -> bool {
	ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
	ch == '_' || ch.is_ascii_alphanumeric()
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

	#[test]
	fn parses_choice_placeholder() {
		let template = parse_snippet_template("${1|a,b,c|}").unwrap();
		assert_eq!(template.nodes.len(), 1);
		let Node::Field(field) = &template.nodes[0] else {
			panic!("expected field node");
		};
		assert_eq!(field.index, 1);
		assert_eq!(field.kind, FieldKind::Choice(vec!["a".to_string(), "b".to_string(), "c".to_string()]));
	}

	#[test]
	fn parses_choice_placeholder_with_escapes() {
		let template = parse_snippet_template(r"${1|a\,b,c\|d,e\\f|}").unwrap();
		let Node::Field(field) = &template.nodes[0] else {
			panic!("expected field node");
		};
		assert_eq!(field.kind, FieldKind::Choice(vec!["a,b".to_string(), "c|d".to_string(), "e\\f".to_string()]));
	}

	#[test]
	fn parses_variable_unbraced() {
		let template = parse_snippet_template("$TM_FILENAME").unwrap();
		assert_eq!(
			template.nodes,
			vec![Node::Var(Var {
				name: "TM_FILENAME".to_string(),
				default: None,
			})]
		);
	}

	#[test]
	fn parses_variable_braced() {
		let template = parse_snippet_template("${TM_FILENAME}").unwrap();
		assert_eq!(
			template.nodes,
			vec![Node::Var(Var {
				name: "TM_FILENAME".to_string(),
				default: None,
			})]
		);
	}

	#[test]
	fn parses_variable_with_default_nodes() {
		let template = parse_snippet_template("${TM_FILENAME:${1:default}}").expect("variable with default should parse");
		let Node::Var(var) = &template.nodes[0] else {
			panic!("expected variable node");
		};
		assert_eq!(var.name, "TM_FILENAME");
		let default = var.default.as_ref().expect("default nodes expected");
		assert_eq!(default.len(), 1);
		assert!(matches!(default[0], Node::Field(_)));
	}

	#[test]
	fn malformed_braced_transform_falls_back_to_literal_text() {
		let template = parse_snippet_template("${1/(.*)/$1").expect("malformed transform should not fail parsing");
		assert_eq!(template.nodes, vec![Node::Text("${1/(.*)/$1".to_string())]);
	}

	#[test]
	fn parses_tabstop_transform() {
		let template = parse_snippet_template("${1/(.*)/$1_bar/gi}").expect("tabstop transform should parse");
		assert_eq!(template.nodes.len(), 1);
		let Node::Transform(transform) = &template.nodes[0] else {
			panic!("expected transform node");
		};
		assert_eq!(transform.source, TransformSource::Tabstop(1));
		assert_eq!(transform.regex, "(.*)");
		assert_eq!(transform.replace, "$1_bar");
		assert_eq!(transform.flags, "gi");
	}

	#[test]
	fn parses_variable_transform() {
		let template = parse_snippet_template("${TM_FILENAME/(.*)/$1_x/}").expect("variable transform should parse");
		let Node::Transform(transform) = &template.nodes[0] else {
			panic!("expected transform node");
		};
		assert_eq!(transform.source, TransformSource::Var("TM_FILENAME".to_string()));
		assert_eq!(transform.regex, "(.*)");
		assert_eq!(transform.replace, "$1_x");
		assert_eq!(transform.flags, "");
	}
}
