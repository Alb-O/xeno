/// Files being evaluated, arranged as a stack.
///
/// The current active file is on the top of the stack.
/// When a file source/import another file, the new file is pushed onto the stack.
/// Attempting to add files that are already in the stack (circular import) results in an error.
///
/// Note that file paths are compared without canonicalization, so the same
/// physical file may still appear multiple times under different paths.
/// This doesn't affect circular import detection though.
#[derive(Debug, Default)]
pub struct FileStack(Vec<PathBuf>);

impl FileStack {
	/// Creates an empty stack.
	pub fn new() -> Self {
		Self(vec![])
	}

	/// Creates a stack with a single file on top.
	///
	/// This is a convenience method that creates an empty stack, then pushes the file onto it.
	/// It skips the circular import check and always succeeds.
	pub fn with_file(path: PathBuf) -> Self {
		Self(vec![path])
	}

	/// Adds a file to the stack.
	///
	/// If the same file is already present in the stack, returns `ParseError::CircularImport`.
	pub fn push(&mut self, path: PathBuf, span: Span) -> Result<(), ParseError> {
		// Check for circular import.
		if let Some(i) = self.0.iter().rposition(|p| p == &path) {
			let filenames: Vec<String> = self.0[i..]
				.iter()
				.chain(std::iter::once(&path))
				.map(|p| p.to_string_lossy().to_string())
				.collect();
			let msg = filenames.join("\nuses ");
			return Err(ParseError::CircularImport(msg, span));
		}

		self.0.push(path);
		Ok(())
	}

	/// Removes a file from the stack and returns its path, or None if the stack is empty.
	pub fn pop(&mut self) -> Option<PathBuf> {
		self.0.pop()
	}

	/// Returns the active file (that is, the file on the top of the stack), or None if the stack is empty.
	pub fn top(&self) -> Option<&Path> {
		self.0.last().map(PathBuf::as_path)
	}

	/// Returns the parent directory of the active file, or None if the stack is empty
	/// or the active file doesn't have a parent directory as part of its path.
	pub fn current_working_directory(&self) -> Option<&Path> {
		self.0.last().and_then(|path| path.parent())
	}
}
