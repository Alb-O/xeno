pub enum ByteStreamSource {
	Read(Box<dyn Read + Send + 'static>),
	File(File),
}

impl ByteStreamSource {
	fn reader(self) -> Option<SourceReader> {
		match self {
			ByteStreamSource::Read(read) => Some(SourceReader::Read(read)),
			ByteStreamSource::File(file) => Some(SourceReader::File(file)),
		}
	}

	/// Source is external to nu, currently unsupported in the vendored build.
	pub fn is_external(&self) -> bool {
		false
	}
}

impl Debug for ByteStreamSource {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ByteStreamSource::Read(_) => f.debug_tuple("Read").field(&"..").finish(),
			ByteStreamSource::File(file) => f.debug_tuple("File").field(file).finish(),
		}
	}
}

enum SourceReader {
	Read(Box<dyn Read + Send + 'static>),
	File(File),
}

impl Read for SourceReader {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		match self {
			SourceReader::Read(reader) => reader.read(buf),
			SourceReader::File(file) => file.read(buf),
		}
	}
}

impl Debug for SourceReader {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			SourceReader::Read(_) => f.debug_tuple("Read").field(&"..").finish(),
			SourceReader::File(file) => f.debug_tuple("File").field(file).finish(),
		}
	}
}

/// Optional type color for [`ByteStream`], which determines type compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ByteStreamType {
	/// Compatible with [`Type::Binary`], and should only be converted to binary, even when the
	/// desired type is unknown.
	Binary,
	/// Compatible with [`Type::String`], and should only be converted to string, even when the
	/// desired type is unknown.
	///
	/// This does not guarantee valid UTF-8 data, but it is conventionally so. Converting to
	/// `String` still requires validation of the data.
	String,
	/// Unknown whether the stream should contain binary or string data. This usually is the result
	/// of an external stream, e.g. an external command or file.
	#[default]
	Unknown,
}

impl ByteStreamType {
	/// Returns the string that describes the byte stream type - i.e., the same as what `describe`
	/// produces. This can be used in type mismatch error messages.
	pub fn describe(self) -> &'static str {
		match self {
			ByteStreamType::Binary => "binary (stream)",
			ByteStreamType::String => "string (stream)",
			ByteStreamType::Unknown => "byte stream",
		}
	}

	/// Returns true if the type is `Binary` or `Unknown`
	pub fn is_binary_coercible(self) -> bool {
		matches!(self, ByteStreamType::Binary | ByteStreamType::Unknown)
	}

	/// Returns true if the type is `String` or `Unknown`
	pub fn is_string_coercible(self) -> bool {
		matches!(self, ByteStreamType::String | ByteStreamType::Unknown)
	}
}

impl From<ByteStreamType> for Type {
	fn from(value: ByteStreamType) -> Self {
		match value {
			ByteStreamType::Binary => Type::Binary,
			ByteStreamType::String => Type::String,
			ByteStreamType::Unknown => Type::Any,
		}
	}
}
