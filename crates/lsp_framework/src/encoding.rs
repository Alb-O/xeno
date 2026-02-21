use lsp_types::PositionEncodingKind;

/// Offset encoding for LSP positions.
///
/// LSP uses UTF-16 by default, but servers can negotiate different encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OffsetEncoding {
	/// UTF-8 byte offsets.
	Utf8,
	/// UTF-16 code unit offsets (LSP default).
	#[default]
	Utf16,
	/// UTF-32 / Unicode codepoint offsets.
	Utf32,
}

impl OffsetEncoding {
	/// Parse from LSP position encoding kind.
	pub fn from_lsp(kind: &PositionEncodingKind) -> Option<Self> {
		match kind.as_str() {
			"utf-8" => Some(Self::Utf8),
			"utf-16" => Some(Self::Utf16),
			"utf-32" => Some(Self::Utf32),
			_ => None,
		}
	}
}
