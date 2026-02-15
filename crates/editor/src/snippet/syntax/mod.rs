//! Snippet template AST and parser for snippet syntax expansion.

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
mod tests;
