impl From<ByteStream> for PipelineData {
	fn from(stream: ByteStream) -> Self {
		Self::byte_stream(stream, None)
	}
}

struct ReadIterator<I>
where
	I: Iterator,
	I::Item: AsRef<[u8]>,
{
	iter: I,
	cursor: Option<Cursor<I::Item>>,
}

impl<I> Read for ReadIterator<I>
where
	I: Iterator,
	I::Item: AsRef<[u8]>,
{
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		while let Some(cursor) = self.cursor.as_mut() {
			let read = cursor.read(buf)?;
			if read == 0 {
				self.cursor = self.iter.next().map(Cursor::new);
			} else {
				return Ok(read);
			}
		}
		Ok(0)
	}
}

struct ReadResultIterator<I, T>
where
	I: Iterator<Item = Result<T, ShellError>>,
	T: AsRef<[u8]>,
{
	iter: I,
	cursor: Option<Cursor<T>>,
}

impl<I, T> Read for ReadResultIterator<I, T>
where
	I: Iterator<Item = Result<T, ShellError>>,
	T: AsRef<[u8]>,
{
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		while let Some(cursor) = self.cursor.as_mut() {
			let read = cursor.read(buf)?;
			if read == 0 {
				self.cursor = self.iter.next().transpose().map_err(ShellErrorBridge)?.map(Cursor::new);
			} else {
				return Ok(read);
			}
		}
		Ok(0)
	}
}

pub struct Reader {
	reader: BufReader<SourceReader>,
	span: Span,
	signals: Signals,
}

impl Reader {
	pub fn span(&self) -> Span {
		self.span
	}
}

impl Read for Reader {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		self.signals.check(&self.span).map_err(ShellErrorBridge)?;
		self.reader.read(buf)
	}
}

impl BufRead for Reader {
	fn fill_buf(&mut self) -> io::Result<&[u8]> {
		self.reader.fill_buf()
	}

	fn consume(&mut self, amt: usize) {
		self.reader.consume(amt)
	}
}

pub struct Lines {
	reader: BufReader<SourceReader>,
	span: Span,
	signals: Signals,
}

impl Lines {
	pub fn span(&self) -> Span {
		self.span
	}
}

impl Iterator for Lines {
	type Item = Result<String, ShellError>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.signals.interrupted() {
			None
		} else {
			let mut buf = Vec::new();
			match self.reader.read_until(b'\n', &mut buf) {
				Ok(0) => None,
				Ok(_) => {
					let Ok(mut string) = String::from_utf8(buf) else {
						return Some(Err(ShellError::NonUtf8 { span: self.span }));
					};
					trim_end_newline(&mut string);
					Some(Ok(string))
				}
				Err(err) => Some(Err(IoError::new(err, self.span, None).into())),
			}
		}
	}
}

pub struct SplitRead {
	internal: SplitReadInner<BufReader<SourceReader>>,
	span: Span,
	signals: Signals,
}

impl SplitRead {
	fn new(reader: SourceReader, delimiter: impl AsRef<[u8]>, span: Span, signals: Signals) -> Self {
		Self {
			internal: SplitReadInner::new(BufReader::new(reader), delimiter),
			span,
			signals,
		}
	}

	pub fn span(&self) -> Span {
		self.span
	}
}

impl Iterator for SplitRead {
	type Item = Result<Vec<u8>, ShellError>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.signals.interrupted() {
			return None;
		}
		self.internal
			.next()
			.map(|r| r.map_err(|err| ShellError::Io(IoError::new_internal(err, "Could not get next value for SplitRead", crate::location!()))))
	}
}

/// Turn a readable stream into [`Value`]s.
///
/// The `Value` type depends on the type of the stream ([`ByteStreamType`]). If `Unknown`, the
/// stream will return strings as long as UTF-8 parsing succeeds, but will start returning binary
/// if it fails.
pub struct Chunks {
	reader: BufReader<SourceReader>,
	pos: u64,
	error: bool,
	span: Span,
	signals: Signals,
	type_: ByteStreamType,
}

impl Chunks {
	fn new(reader: SourceReader, span: Span, signals: Signals, type_: ByteStreamType) -> Self {
		Self {
			reader: BufReader::new(reader),
			pos: 0,
			error: false,
			span,
			signals,
			type_,
		}
	}

	pub fn span(&self) -> Span {
		self.span
	}

	fn next_string(&mut self) -> Result<Option<String>, (Vec<u8>, ShellError)> {
		let from_io_error = |err: std::io::Error| match ShellErrorBridge::try_from(err) {
			Ok(err) => err.0,
			Err(err) => IoError::new(err, self.span, None).into(),
		};

		// Get some data from the reader
		let buf = self.reader.fill_buf().map_err(from_io_error).map_err(|err| (vec![], err))?;

		// If empty, this is EOF
		if buf.is_empty() {
			return Ok(None);
		}

		let mut buf = buf.to_vec();
		let mut consumed = 0;

		// If the buf length is under 4 bytes, it could be invalid, so try to get more
		if buf.len() < 4 {
			consumed += buf.len();
			self.reader.consume(buf.len());
			match self.reader.fill_buf() {
				Ok(more_bytes) => buf.extend_from_slice(more_bytes),
				Err(err) => return Err((buf, from_io_error(err))),
			}
		}

		// Try to parse utf-8 and decide what to do
		match String::from_utf8(buf) {
			Ok(string) => {
				self.reader.consume(string.len() - consumed);
				self.pos += string.len() as u64;
				Ok(Some(string))
			}
			Err(err) if err.utf8_error().error_len().is_none() => {
				// There is some valid data at the beginning, and this is just incomplete, so just
				// consume that and return it
				let valid_up_to = err.utf8_error().valid_up_to();
				if valid_up_to > consumed {
					self.reader.consume(valid_up_to - consumed);
				}
				let mut buf = err.into_bytes();
				buf.truncate(valid_up_to);
				buf.shrink_to_fit();
				let string = String::from_utf8(buf).expect("failed to parse utf-8 even after correcting error");
				self.pos += string.len() as u64;
				Ok(Some(string))
			}
			Err(err) => {
				// There is an error at the beginning and we have no hope of parsing further.
				let shell_error = ShellError::NonUtf8Custom {
					msg: format!("invalid utf-8 sequence starting at index {}", self.pos),
					span: self.span,
				};
				let buf = err.into_bytes();
				// We are consuming the entire buf though, because we're returning it in case it
				// will be cast to binary
				if buf.len() > consumed {
					self.reader.consume(buf.len() - consumed);
				}
				self.pos += buf.len() as u64;
				Err((buf, shell_error))
			}
		}
	}
}

impl Iterator for Chunks {
	type Item = Result<Value, ShellError>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.error || self.signals.interrupted() {
			None
		} else {
			match self.type_ {
				// Binary should always be binary
				ByteStreamType::Binary => {
					let buf = match self.reader.fill_buf() {
						Ok(buf) => buf,
						Err(err) => {
							self.error = true;
							return Some(Err(ShellError::Io(IoError::new(err, self.span, None))));
						}
					};
					if !buf.is_empty() {
						let len = buf.len();
						let value = Value::binary(buf, self.span);
						self.reader.consume(len);
						self.pos += len as u64;
						Some(Ok(value))
					} else {
						None
					}
				}
				// String produces an error if UTF-8 can't be parsed
				ByteStreamType::String => match self.next_string().transpose()? {
					Ok(string) => Some(Ok(Value::string(string, self.span))),
					Err((_, err)) => {
						self.error = true;
						Some(Err(err))
					}
				},
				// For Unknown, we try to create strings, but we switch to binary mode if we
				// fail
				ByteStreamType::Unknown => {
					match self.next_string().transpose()? {
						Ok(string) => Some(Ok(Value::string(string, self.span))),
						Err((buf, _)) if !buf.is_empty() => {
							// Switch to binary mode
							self.type_ = ByteStreamType::Binary;
							Some(Ok(Value::binary(buf, self.span)))
						}
						Err((_, err)) => {
							self.error = true;
							Some(Err(err))
						}
					}
				}
			}
		}
	}
}

fn trim_end_newline(string: &mut String) {
	if string.ends_with('\n') {
		string.pop();
		if string.ends_with('\r') {
			string.pop();
		}
	}
}

const DEFAULT_BUF_SIZE: usize = 8192;

pub fn copy_with_signals(mut reader: impl Read, mut writer: impl Write, span: Span, signals: &Signals) -> Result<u64, ShellError> {
	let from_io_error = IoError::factory(span, None);
	if signals.is_empty() {
		match io::copy(&mut reader, &mut writer) {
			Ok(n) => {
				writer.flush().map_err(&from_io_error)?;
				Ok(n)
			}
			Err(err) => {
				let _ = writer.flush();
				match ShellErrorBridge::try_from(err) {
					Ok(ShellErrorBridge(shell_error)) => Err(shell_error),
					Err(err) => Err(from_io_error(err).into()),
				}
			}
		}
	} else {
		// #[cfg(any(target_os = "linux", target_os = "android"))]
		// {
		//     return crate::sys::kernel_copy::copy_spec(reader, writer);
		// }
		match generic_copy(&mut reader, &mut writer, span, signals) {
			Ok(len) => {
				writer.flush().map_err(&from_io_error)?;
				Ok(len)
			}
			Err(err) => {
				let _ = writer.flush();
				Err(err)
			}
		}
	}
}

// Copied from [`std::io::copy`]
fn generic_copy(mut reader: impl Read, mut writer: impl Write, span: Span, signals: &Signals) -> Result<u64, ShellError> {
	let from_io_error = IoError::factory(span, None);
	let buf = &mut [0; DEFAULT_BUF_SIZE];
	let mut len = 0;
	loop {
		signals.check(&span)?;
		let n = match reader.read(buf) {
			Ok(0) => break,
			Ok(n) => n,
			Err(e) if e.kind() == ErrorKind::Interrupted => continue,
			Err(e) => match ShellErrorBridge::try_from(e) {
				Ok(ShellErrorBridge(e)) => return Err(e),
				Err(e) => return Err(from_io_error(e).into()),
			},
		};
		len += n;
		writer.write_all(&buf[..n]).map_err(&from_io_error)?;
	}
	Ok(len as u64)
}

struct ReadGenerator<F>
where
	F: FnMut(&mut Vec<u8>) -> Result<bool, ShellError> + Send + 'static,
{
	buffer: Cursor<Vec<u8>>,
	generator: F,
}

impl<F> BufRead for ReadGenerator<F>
where
	F: FnMut(&mut Vec<u8>) -> Result<bool, ShellError> + Send + 'static,
{
	fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
		// We have to loop, because it's important that we don't leave the buffer empty unless we're
		// truly at the end of the stream.
		while self.buffer.fill_buf()?.is_empty() {
			// Reset the cursor to the beginning and truncate
			self.buffer.set_position(0);
			self.buffer.get_mut().clear();
			// Ask the generator to generate data
			if !(self.generator)(self.buffer.get_mut()).map_err(ShellErrorBridge)? {
				// End of stream
				break;
			}
		}
		self.buffer.fill_buf()
	}

	fn consume(&mut self, amt: usize) {
		self.buffer.consume(amt);
	}
}

impl<F> Read for ReadGenerator<F>
where
	F: FnMut(&mut Vec<u8>) -> Result<bool, ShellError> + Send + 'static,
{
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		// Straightforward implementation on top of BufRead
		let slice = self.fill_buf()?;
		let len = buf.len().min(slice.len());
		buf[..len].copy_from_slice(&slice[..len]);
		self.consume(len);
		Ok(len)
	}
}
