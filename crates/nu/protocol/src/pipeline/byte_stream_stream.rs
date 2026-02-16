/// A potentially infinite, interruptible stream of bytes.
///
/// To create a [`ByteStream`], you can use any of the following methods:
/// - [`read`](ByteStream::read): takes any type that implements [`Read`].
/// - [`file`](ByteStream::file): takes a [`File`].
/// - [`from_iter`](ByteStream::from_iter): takes an [`Iterator`] whose items implement `AsRef<[u8]>`.
/// - [`from_result_iter`](ByteStream::from_result_iter): same as [`from_iter`](ByteStream::from_iter),
///   but each item is a `Result<T, ShellError>`.
/// - [`from_fn`](ByteStream::from_fn): uses a generator function to fill a buffer whenever it is
///   empty. This has high performance because it doesn't need to allocate for each chunk of data,
///   and can just reuse the same buffer.
///
/// Byte streams have a [type](.type_()) which is used to preserve type compatibility when they
/// are the result of an internal command. It is important that this be set to the correct value.
/// [`Unknown`](ByteStreamType::Unknown) is used only for external sources where the type can not
/// be inherently determined, and having it automatically act as a string or binary depending on
/// whether it parses as UTF-8 or not is desirable.
///
/// The data of a [`ByteStream`] can be accessed using one of the following methods:
/// - [`reader`](ByteStream::reader): returns a [`Read`]-able type to get the raw bytes in the stream.
/// - [`lines`](ByteStream::lines): splits the bytes on lines and returns an [`Iterator`]
///   where each item is a `Result<String, ShellError>`.
/// - [`chunks`](ByteStream::chunks): returns an [`Iterator`] of [`Value`]s where each value is
///   either a string or binary.
///   Try not to use this method if possible. Rather, please use [`reader`](ByteStream::reader)
///   (or [`lines`](ByteStream::lines) if it matches the situation).
///
/// Additionally, there are few methods to collect a [`ByteStream`] into memory:
/// - [`into_bytes`](ByteStream::into_bytes): collects all bytes into a [`Vec<u8>`].
/// - [`into_string`](ByteStream::into_string): collects all bytes into a [`String`], erroring if utf-8 decoding failed.
/// - [`into_value`](ByteStream::into_value): collects all bytes into a value typed appropriately
///   for the [type](.type_()) of this stream. If the type is [`Unknown`](ByteStreamType::Unknown),
///   it will produce a string value if the data is valid UTF-8, or a binary value otherwise.
///
/// There are also a few other methods to consume all the data of a [`ByteStream`]:
/// - [`drain`](ByteStream::drain): consumes all bytes and outputs nothing.
/// - [`write_to`](ByteStream::write_to): writes all bytes to the given [`Write`] destination.
/// - [`print`](ByteStream::print): a convenience wrapper around [`write_to`](ByteStream::write_to).
///   It prints all bytes to stdout or stderr.
///
/// Internally, [`ByteStream`]s currently come in three flavors according to [`ByteStreamSource`].
/// See its documentation for more information.
#[derive(Debug)]
pub struct ByteStream {
	stream: ByteStreamSource,
	span: Span,
	signals: Signals,
	type_: ByteStreamType,
	known_size: Option<u64>,
	caller_spans: Vec<Span>,
}

impl ByteStream {
	/// Create a new [`ByteStream`] from a [`ByteStreamSource`].
	pub fn new(stream: ByteStreamSource, span: Span, signals: Signals, type_: ByteStreamType) -> Self {
		Self {
			stream,
			span,
			signals,
			type_,
			known_size: None,
			caller_spans: vec![],
		}
	}

	/// Push a caller [`Span`] to the bytestream, it's useful to construct a backtrace.
	pub fn push_caller_span(&mut self, span: Span) {
		if span != self.span {
			self.caller_spans.push(span)
		}
	}

	/// Get all caller [`Span`], it's useful to construct a backtrace.
	pub fn get_caller_spans(&self) -> &Vec<Span> {
		&self.caller_spans
	}

	/// Create a [`ByteStream`] from an arbitrary reader. The type must be provided.
	pub fn read(reader: impl Read + Send + 'static, span: Span, signals: Signals, type_: ByteStreamType) -> Self {
		Self::new(ByteStreamSource::Read(Box::new(reader)), span, signals, type_)
	}

	pub fn skip(self, span: Span, n: u64) -> Result<Self, ShellError> {
		let known_size = self.known_size.map(|len| len.saturating_sub(n));
		if let Some(mut reader) = self.reader() {
			// Copy the number of skipped bytes into the sink before proceeding
			io::copy(&mut (&mut reader).take(n), &mut io::sink()).map_err(|err| IoError::new(err, span, None))?;
			Ok(ByteStream::read(reader, span, Signals::empty(), ByteStreamType::Binary).with_known_size(known_size))
		} else {
			Err(ShellError::TypeMismatch {
				err_message: "expected readable stream".into(),
				span,
			})
		}
	}

	pub fn take(self, span: Span, n: u64) -> Result<Self, ShellError> {
		let known_size = self.known_size.map(|s| s.min(n));
		if let Some(reader) = self.reader() {
			Ok(ByteStream::read(reader.take(n), span, Signals::empty(), ByteStreamType::Binary).with_known_size(known_size))
		} else {
			Err(ShellError::TypeMismatch {
				err_message: "expected readable stream".into(),
				span,
			})
		}
	}

	pub fn slice(self, val_span: Span, call_span: Span, range: IntRange) -> Result<Self, ShellError> {
		if let Some(len) = self.known_size {
			let start = range.absolute_start(len);
			let stream = self.skip(val_span, start);

			match range.absolute_end(len) {
				Bound::Unbounded => stream,
				Bound::Included(end) | Bound::Excluded(end) if end < start => stream.and_then(|s| s.take(val_span, 0)),
				Bound::Included(end) => {
					let distance = end - start + 1;
					stream.and_then(|s| s.take(val_span, distance.min(len)))
				}
				Bound::Excluded(end) => {
					let distance = end - start;
					stream.and_then(|s| s.take(val_span, distance.min(len)))
				}
			}
		} else if range.is_relative() {
			Err(ShellError::RelativeRangeOnInfiniteStream { span: call_span })
		} else {
			let start = range.start() as u64;
			let stream = self.skip(val_span, start);

			match range.distance() {
				Bound::Unbounded => stream,
				Bound::Included(distance) => stream.and_then(|s| s.take(val_span, distance + 1)),
				Bound::Excluded(distance) => stream.and_then(|s| s.take(val_span, distance)),
			}
		}
	}

	/// Create a [`ByteStream`] from a string. The type of the stream is always `String`.
	pub fn read_string(string: String, span: Span, signals: Signals) -> Self {
		let len = string.len();
		ByteStream::read(Cursor::new(string.into_bytes()), span, signals, ByteStreamType::String).with_known_size(Some(len as u64))
	}

	/// Create a [`ByteStream`] from a byte vector. The type of the stream is always `Binary`.
	pub fn read_binary(bytes: Vec<u8>, span: Span, signals: Signals) -> Self {
		let len = bytes.len();
		ByteStream::read(Cursor::new(bytes), span, signals, ByteStreamType::Binary).with_known_size(Some(len as u64))
	}

	/// Create a [`ByteStream`] from a file.
	///
	/// The type is implicitly `Unknown`, as it's not typically known whether files will
	/// return text or binary.
	pub fn file(file: File, span: Span, signals: Signals) -> Self {
		Self::new(ByteStreamSource::File(file), span, signals, ByteStreamType::Unknown)
	}

	/// Create a [`ByteStream`] that reads from stdin.
	///
	/// The type is implicitly `Unknown`, as it's not typically known whether stdin is text or
	/// binary.
	pub fn stdin(span: Span) -> Result<Self, ShellError> {
		Err(ShellError::DisabledOsSupport {
			msg: "Stdin is not supported".to_string(),
			span,
		})
	}

	/// Create a [`ByteStream`] from a generator function that writes data to the given buffer
	/// when called, and returns `Ok(false)` on end of stream.
	pub fn from_fn(
		span: Span,
		signals: Signals,
		type_: ByteStreamType,
		generator: impl FnMut(&mut Vec<u8>) -> Result<bool, ShellError> + Send + 'static,
	) -> Self {
		Self::read(
			ReadGenerator {
				buffer: Cursor::new(Vec::new()),
				generator,
			},
			span,
			signals,
			type_,
		)
	}

	pub fn with_type(mut self, type_: ByteStreamType) -> Self {
		self.type_ = type_;
		self
	}

	/// Create a new [`ByteStream`] from an [`Iterator`] of bytes slices.
	///
	/// The returned [`ByteStream`] will have a [`ByteStreamSource`] of `Read`.
	pub fn from_iter<I>(iter: I, span: Span, signals: Signals, type_: ByteStreamType) -> Self
	where
		I: IntoIterator,
		I::IntoIter: Send + 'static,
		I::Item: AsRef<[u8]> + Default + Send + 'static,
	{
		let iter = iter.into_iter();
		let cursor = Some(Cursor::new(I::Item::default()));
		Self::read(ReadIterator { iter, cursor }, span, signals, type_)
	}

	/// Create a new [`ByteStream`] from an [`Iterator`] of [`Result`] bytes slices.
	///
	/// The returned [`ByteStream`] will have a [`ByteStreamSource`] of `Read`.
	pub fn from_result_iter<I, T>(iter: I, span: Span, signals: Signals, type_: ByteStreamType) -> Self
	where
		I: IntoIterator<Item = Result<T, ShellError>>,
		I::IntoIter: Send + 'static,
		T: AsRef<[u8]> + Default + Send + 'static,
	{
		let iter = iter.into_iter();
		let cursor = Some(Cursor::new(T::default()));
		Self::read(ReadResultIterator { iter, cursor }, span, signals, type_)
	}

	/// Set the known size, in number of bytes, of the [`ByteStream`].
	pub fn with_known_size(mut self, size: Option<u64>) -> Self {
		self.known_size = size;
		self
	}

	/// Get a reference to the inner [`ByteStreamSource`] of the [`ByteStream`].
	pub fn source(&self) -> &ByteStreamSource {
		&self.stream
	}

	/// Get a mutable reference to the inner [`ByteStreamSource`] of the [`ByteStream`].
	pub fn source_mut(&mut self) -> &mut ByteStreamSource {
		&mut self.stream
	}

	/// Returns the [`Span`] associated with the [`ByteStream`].
	pub fn span(&self) -> Span {
		self.span
	}

	/// Changes the [`Span`] associated with the [`ByteStream`].
	pub fn with_span(mut self, span: Span) -> Self {
		self.span = span;
		self
	}

	/// Returns the [`ByteStreamType`] associated with the [`ByteStream`].
	pub fn type_(&self) -> ByteStreamType {
		self.type_
	}

	/// Returns the known size, in number of bytes, of the [`ByteStream`].
	pub fn known_size(&self) -> Option<u64> {
		self.known_size
	}

	/// Convert the [`ByteStream`] into its [`Reader`] which allows one to [`Read`] the raw bytes of the stream.
	///
	/// [`Reader`] is buffered and also implements [`BufRead`].
	///
	/// If the source of the [`ByteStream`] is [`ByteStreamSource::Child`] and the child has no stdout,
	/// then the stream is considered empty and `None` will be returned.
	pub fn reader(self) -> Option<Reader> {
		let reader = self.stream.reader()?;
		Some(Reader {
			reader: BufReader::new(reader),
			span: self.span,
			signals: self.signals,
		})
	}

	/// Convert the [`ByteStream`] into a [`Lines`] iterator where each element is a `Result<String, ShellError>`.
	///
	/// There is no limit on how large each line will be. Ending new lines (`\n` or `\r\n`) are
	/// stripped from each line. If a line fails to be decoded as utf-8, then it will become a [`ShellError`].
	///
	/// If the source of the [`ByteStream`] is [`ByteStreamSource::Child`] and the child has no stdout,
	/// then the stream is considered empty and `None` will be returned.
	pub fn lines(self) -> Option<Lines> {
		let reader = self.stream.reader()?;
		Some(Lines {
			reader: BufReader::new(reader),
			span: self.span,
			signals: self.signals,
		})
	}

	/// Convert the [`ByteStream`] into a [`SplitRead`] iterator where each element is a `Result<String, ShellError>`.
	///
	/// Each call to [`next`](Iterator::next) reads the currently available data from the byte
	/// stream source, until `delimiter` or the end of the stream is encountered.
	///
	/// If the source of the [`ByteStream`] is [`ByteStreamSource::Child`] and the child has no stdout,
	/// then the stream is considered empty and `None` will be returned.
	pub fn split(self, delimiter: Vec<u8>) -> Option<SplitRead> {
		let reader = self.stream.reader()?;
		Some(SplitRead::new(reader, delimiter, self.span, self.signals))
	}

	/// Convert the [`ByteStream`] into a [`Chunks`] iterator where each element is a `Result<Value, ShellError>`.
	///
	/// Each call to [`next`](Iterator::next) reads the currently available data from the byte stream source,
	/// up to a maximum size. The values are typed according to the [type](.type_()) of the
	/// stream, and if that type is [`Unknown`](ByteStreamType::Unknown), string values will be
	/// produced as long as the stream continues to parse as valid UTF-8, but binary values will
	/// be produced instead of the stream fails to parse as UTF-8 instead at any point.
	/// Any and all newlines are kept intact in each chunk.
	///
	/// Where possible, prefer [`reader`](ByteStream::reader) or [`lines`](ByteStream::lines) over this method.
	/// Those methods are more likely to be used in a semantically correct way
	/// (and [`reader`](ByteStream::reader) is more efficient too).
	///
	/// If the source of the [`ByteStream`] is [`ByteStreamSource::Child`] and the child has no stdout,
	/// then the stream is considered empty and `None` will be returned.
	pub fn chunks(self) -> Option<Chunks> {
		let reader = self.stream.reader()?;
		Some(Chunks::new(reader, self.span, self.signals, self.type_))
	}

	/// Convert the [`ByteStream`] into its inner [`ByteStreamSource`].
	pub fn into_source(self) -> ByteStreamSource {
		self.stream
	}

	/// Attempt to convert the [`ByteStream`] into a [`Stdio`].
	///
	/// This will succeed if the [`ByteStreamSource`] of the [`ByteStream`] is either:
	/// * [`File`](ByteStreamSource::File)
	///
	/// All other cases return an `Err` with the original [`ByteStream`] in it.
	pub fn into_stdio(self) -> Result<Stdio, Self> {
		match self.stream {
			ByteStreamSource::Read(..) => Err(self),
			ByteStreamSource::File(file) => Ok(file.into()),
		}
	}

	/// Collect all the bytes of the [`ByteStream`] into a [`Vec<u8>`].
	///
	/// Any trailing new lines are kept in the returned [`Vec`].
	pub fn into_bytes(self) -> Result<Vec<u8>, ShellError> {
		// todo!() ctrlc
		let from_io_error = IoError::factory(self.span, None);
		match self.stream {
			ByteStreamSource::Read(mut read) => {
				let mut buf = Vec::new();
				read.read_to_end(&mut buf).map_err(|err| match ShellErrorBridge::try_from(err) {
					Ok(ShellErrorBridge(err)) => err,
					Err(err) => ShellError::Io(from_io_error(err)),
				})?;
				Ok(buf)
			}
			ByteStreamSource::File(mut file) => {
				let mut buf = Vec::new();
				file.read_to_end(&mut buf).map_err(&from_io_error)?;
				Ok(buf)
			}
		}
	}

	/// Collect the stream into a `String` in-memory. This can only succeed if the data contained is
	/// valid UTF-8.
	///
	/// The trailing new line (`\n` or `\r\n`), if any, is removed from the [`String`] prior to
	/// being returned, if this is a stream coming from an external process or file.
	///
	/// If the [type](.type_()) is specified as `Binary`, this operation always fails, even if the
	/// data would have been valid UTF-8.
	pub fn into_string(self) -> Result<String, ShellError> {
		let span = self.span;
		if self.type_.is_string_coercible() {
			let trim = self.stream.is_external();
			let bytes = self.into_bytes()?;
			let mut string = String::from_utf8(bytes).map_err(|err| ShellError::NonUtf8Custom { span, msg: err.to_string() })?;
			if trim {
				trim_end_newline(&mut string);
			}
			Ok(string)
		} else {
			Err(ShellError::TypeMismatch {
				err_message: "expected string, but got binary".into(),
				span,
			})
		}
	}

	/// Collect all the bytes of the [`ByteStream`] into a [`Value`].
	///
	/// If this is a `String` stream, the stream is decoded to UTF-8. If the stream came from an
	/// external process or file, the trailing new line (`\n` or `\r\n`), if any, is removed from
	/// the [`String`] prior to being returned.
	///
	/// If this is a `Binary` stream, a [`Value::Binary`] is returned with any trailing new lines
	/// preserved.
	///
	/// If this is an `Unknown` stream, the behavior depends on whether the stream parses as valid
	/// UTF-8 or not. If it does, this is uses the `String` behavior; if not, it uses the `Binary`
	/// behavior.
	pub fn into_value(self) -> Result<Value, ShellError> {
		let span = self.span;
		let trim = self.stream.is_external();
		let value = match self.type_ {
			// If the type is specified, then the stream should always become that type:
			ByteStreamType::Binary => Value::binary(self.into_bytes()?, span),
			ByteStreamType::String => Value::string(self.into_string()?, span),
			// If the type is not specified, then it just depends on whether it parses or not:
			ByteStreamType::Unknown => match String::from_utf8(self.into_bytes()?) {
				Ok(mut str) => {
					if trim {
						trim_end_newline(&mut str);
					}
					Value::string(str, span)
				}
				Err(err) => Value::binary(err.into_bytes(), span),
			},
		};
		Ok(value)
	}

	/// Consume and drop all bytes of the [`ByteStream`].
	pub fn drain(self) -> Result<(), ShellError> {
		match self.stream {
			ByteStreamSource::Read(read) => {
				copy_with_signals(read, io::sink(), self.span, &self.signals)?;
				Ok(())
			}
			ByteStreamSource::File(_) => Ok(()),
		}
	}

	/// Print all bytes of the [`ByteStream`] to stdout or stderr.
	pub fn print(self, to_stderr: bool) -> Result<(), ShellError> {
		if to_stderr {
			self.write_to(&mut io::stderr())
		} else {
			self.write_to(&mut io::stdout())
		}
	}

	/// Write all bytes of the [`ByteStream`] to `dest`.
	pub fn write_to(self, dest: impl Write) -> Result<(), ShellError> {
		let span = self.span;
		let signals = &self.signals;
		match self.stream {
			ByteStreamSource::Read(read) => {
				copy_with_signals(read, dest, span, signals)?;
			}
			ByteStreamSource::File(file) => {
				copy_with_signals(file, dest, span, signals)?;
			}
		}
		Ok(())
	}
}
