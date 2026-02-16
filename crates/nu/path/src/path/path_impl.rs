impl<Form: PathForm> Path<Form> {
	/// Create a new path of any form without validating invariants.
	#[inline]
	fn new_unchecked<P: AsRef<OsStr> + ?Sized>(path: &P) -> &Self {
		#[ref_cast_custom]
		fn ref_cast<Form: PathForm>(path: &std::path::Path) -> &Path<Form>;

		debug_assert!(Form::invariants_satisfied(path));
		ref_cast(std::path::Path::new(path))
	}

	/// Attempt to create a new [`Path`] from a reference of another type.
	///
	/// This is a convenience method instead of having to use `try_into` with a type annotation.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::{AbsolutePath, RelativePath};
	///
	/// assert!(AbsolutePath::try_new("foo.txt").is_err());
	/// assert!(RelativePath::try_new("foo.txt").is_ok());
	/// ```
	#[inline]
	pub fn try_new<'a, T>(path: &'a T) -> Result<&'a Self, <&'a T as TryInto<&'a Self>>::Error>
	where
		T: ?Sized,
		&'a T: TryInto<&'a Self>,
	{
		path.try_into()
	}

	/// Returns the underlying [`OsStr`] slice.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let os_str = Path::new("foo.txt").as_os_str();
	/// assert_eq!(os_str, std::ffi::OsStr::new("foo.txt"));
	/// ```
	#[must_use]
	#[inline]
	pub fn as_os_str(&self) -> &OsStr {
		self.inner.as_os_str()
	}

	/// Returns a [`str`] slice if the [`Path`] is valid unicode.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let path = Path::new("foo.txt");
	/// assert_eq!(path.to_str(), Some("foo.txt"));
	/// ```
	#[inline]
	pub fn to_str(&self) -> Option<&str> {
		self.inner.to_str()
	}

	/// Converts a [`Path`] to a `Cow<str>`.
	///
	/// Any non-Unicode sequences are replaced with `U+FFFD REPLACEMENT CHARACTER`.
	///
	/// # Examples
	///
	/// Calling `to_string_lossy` on a [`Path`] with valid unicode:
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let path = Path::new("foo.txt");
	/// assert_eq!(path.to_string_lossy(), "foo.txt");
	/// ```
	///
	/// Had `path` contained invalid unicode, the `to_string_lossy` call might have returned
	/// `"fo�.txt"`.
	#[inline]
	pub fn to_string_lossy(&self) -> Cow<'_, str> {
		self.inner.to_string_lossy()
	}

	/// Converts a [`Path`] to an owned [`PathBuf`].
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::{Path, PathBuf};
	///
	/// let path_buf = Path::new("foo.txt").to_path_buf();
	/// assert_eq!(path_buf, PathBuf::from("foo.txt"));
	/// ```
	#[inline]
	pub fn to_path_buf(&self) -> PathBuf<Form> {
		PathBuf::new_unchecked(self.inner.to_path_buf())
	}

	/// Returns the [`Path`] without its final component, if there is one.
	///
	/// This means it returns `Some("")` for relative paths with one component.
	///
	/// Returns [`None`] if the path terminates in a root or prefix, or if it's
	/// the empty string.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let path = Path::new("/foo/bar");
	/// let parent = path.parent().unwrap();
	/// assert_eq!(parent, Path::new("/foo"));
	///
	/// let grand_parent = parent.parent().unwrap();
	/// assert_eq!(grand_parent, Path::new("/"));
	/// assert_eq!(grand_parent.parent(), None);
	///
	/// let relative_path = Path::new("foo/bar");
	/// let parent = relative_path.parent();
	/// assert_eq!(parent, Some(Path::new("foo")));
	/// let grand_parent = parent.and_then(Path::parent);
	/// assert_eq!(grand_parent, Some(Path::new("")));
	/// let great_grand_parent = grand_parent.and_then(Path::parent);
	/// assert_eq!(great_grand_parent, None);
	/// ```
	#[must_use]
	#[inline]
	pub fn parent(&self) -> Option<&Self> {
		self.inner.parent().map(Self::new_unchecked)
	}

	/// Produces an iterator over a [`Path`] and its ancestors.
	///
	/// The iterator will yield the [`Path`] that is returned if the [`parent`](Path::parent) method
	/// is used zero or more times. That means, the iterator will yield `&self`,
	/// `&self.parent().unwrap()`, `&self.parent().unwrap().parent().unwrap()` and so on.
	/// If the [`parent`](Path::parent) method returns [`None`], the iterator will do likewise.
	/// The iterator will always yield at least one value, namely `&self`.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let mut ancestors = Path::new("/foo/bar").ancestors();
	/// assert_eq!(ancestors.next(), Some(Path::new("/foo/bar")));
	/// assert_eq!(ancestors.next(), Some(Path::new("/foo")));
	/// assert_eq!(ancestors.next(), Some(Path::new("/")));
	/// assert_eq!(ancestors.next(), None);
	///
	/// let mut ancestors = Path::new("../foo/bar").ancestors();
	/// assert_eq!(ancestors.next(), Some(Path::new("../foo/bar")));
	/// assert_eq!(ancestors.next(), Some(Path::new("../foo")));
	/// assert_eq!(ancestors.next(), Some(Path::new("..")));
	/// assert_eq!(ancestors.next(), Some(Path::new("")));
	/// assert_eq!(ancestors.next(), None);
	/// ```
	#[inline]
	pub fn ancestors(&self) -> Ancestors<'_, Form> {
		Ancestors {
			_form: PhantomData,
			inner: self.inner.ancestors(),
		}
	}

	/// Returns the final component of a [`Path`], if there is one.
	///
	/// If the path is a normal file, this is the file name. If it's the path of a directory, this
	/// is the directory name.
	///
	/// Returns [`None`] if the path terminates in `..`.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	/// use std::ffi::OsStr;
	///
	/// assert_eq!(Some(OsStr::new("bin")), Path::new("/usr/bin/").file_name());
	/// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("tmp/foo.txt").file_name());
	/// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("foo.txt/.").file_name());
	/// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("foo.txt/.//").file_name());
	/// assert_eq!(None, Path::new("foo.txt/..").file_name());
	/// assert_eq!(None, Path::new("/").file_name());
	/// ```
	#[must_use]
	#[inline]
	pub fn file_name(&self) -> Option<&OsStr> {
		self.inner.file_name()
	}

	/// Returns a relative path that, when joined onto `base`, yields `self`.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::{Path, PathBuf};
	///
	/// let path = Path::new("/test/haha/foo.txt");
	///
	/// assert_eq!(path.strip_prefix("/").unwrap(), Path::new("test/haha/foo.txt"));
	/// assert_eq!(path.strip_prefix("/test").unwrap(), Path::new("haha/foo.txt"));
	/// assert_eq!(path.strip_prefix("/test/").unwrap(), Path::new("haha/foo.txt"));
	/// assert_eq!(path.strip_prefix("/test/haha/foo.txt").unwrap(), Path::new(""));
	/// assert_eq!(path.strip_prefix("/test/haha/foo.txt/").unwrap(), Path::new(""));
	///
	/// assert!(path.strip_prefix("test").is_err());
	/// assert!(path.strip_prefix("/haha").is_err());
	///
	/// let prefix = PathBuf::from("/test/");
	/// assert_eq!(path.strip_prefix(prefix).unwrap(), Path::new("haha/foo.txt"));
	/// ```
	#[inline]
	pub fn strip_prefix(&self, base: impl AsRef<Path>) -> Result<&RelativePath, StripPrefixError> {
		self.inner.strip_prefix(&base.as_ref().inner).map(RelativePath::new_unchecked)
	}

	/// Determines whether `base` is a prefix of `self`.
	///
	/// Only considers whole path components to match.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let path = Path::new("/etc/passwd");
	///
	/// assert!(path.starts_with("/etc"));
	/// assert!(path.starts_with("/etc/"));
	/// assert!(path.starts_with("/etc/passwd"));
	/// assert!(path.starts_with("/etc/passwd/")); // extra slash is okay
	/// assert!(path.starts_with("/etc/passwd///")); // multiple extra slashes are okay
	///
	/// assert!(!path.starts_with("/e"));
	/// assert!(!path.starts_with("/etc/passwd.txt"));
	///
	/// assert!(!Path::new("/etc/foo.rs").starts_with("/etc/foo"));
	/// ```
	#[must_use]
	#[inline]
	pub fn starts_with(&self, base: impl AsRef<Path>) -> bool {
		self.inner.starts_with(&base.as_ref().inner)
	}

	/// Determines whether `child` is a suffix of `self`.
	///
	/// Only considers whole path components to match.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let path = Path::new("/etc/resolv.conf");
	///
	/// assert!(path.ends_with("resolv.conf"));
	/// assert!(path.ends_with("etc/resolv.conf"));
	/// assert!(path.ends_with("/etc/resolv.conf"));
	///
	/// assert!(!path.ends_with("/resolv.conf"));
	/// assert!(!path.ends_with("conf")); // use .extension() instead
	/// ```
	#[must_use]
	#[inline]
	pub fn ends_with(&self, child: impl AsRef<Path>) -> bool {
		self.inner.ends_with(&child.as_ref().inner)
	}

	/// Extracts the stem (non-extension) portion of [`self.file_name`](Path::file_name).
	///
	/// The stem is:
	///
	/// * [`None`], if there is no file name;
	/// * The entire file name if there is no embedded `.`;
	/// * The entire file name if the file name begins with `.` and has no other `.`s within;
	/// * Otherwise, the portion of the file name before the final `.`
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// assert_eq!("foo", Path::new("foo.rs").file_stem().unwrap());
	/// assert_eq!("foo.tar", Path::new("foo.tar.gz").file_stem().unwrap());
	/// ```
	#[must_use]
	#[inline]
	pub fn file_stem(&self) -> Option<&OsStr> {
		self.inner.file_stem()
	}

	/// Extracts the extension (without the leading dot) of [`self.file_name`](Path::file_name),
	/// if possible.
	///
	/// The extension is:
	///
	/// * [`None`], if there is no file name;
	/// * [`None`], if there is no embedded `.`;
	/// * [`None`], if the file name begins with `.` and has no other `.`s within;
	/// * Otherwise, the portion of the file name after the final `.`
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// assert_eq!("rs", Path::new("foo.rs").extension().unwrap());
	/// assert_eq!("gz", Path::new("foo.tar.gz").extension().unwrap());
	/// ```
	#[must_use]
	#[inline]
	pub fn extension(&self) -> Option<&OsStr> {
		self.inner.extension()
	}

	/// Produces an iterator over the [`Component`](std::path::Component)s of the path.
	///
	/// When parsing the path, there is a small amount of normalization:
	///
	/// * Repeated separators are ignored, so `a/b` and `a//b` both have
	///   `a` and `b` as components.
	///
	/// * Occurrences of `.` are normalized away, except if they are at the
	///   beginning of the path. For example, `a/./b`, `a/b/`, `a/b/.` and
	///   `a/b` all have `a` and `b` as components, but `./a/b` starts with
	///   an additional [`CurDir`](std::path::Component) component.
	///
	/// * A trailing slash is normalized away, `/a/b` and `/a/b/` are equivalent.
	///
	/// Note that no other normalization takes place; in particular, `a/c`
	/// and `a/b/../c` are distinct, to account for the possibility that `b`
	/// is a symbolic link (so its parent isn't `a`).
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	/// use std::path::Component;
	/// use std::ffi::OsStr;
	///
	/// let mut components = Path::new("/tmp/foo.txt").components();
	///
	/// assert_eq!(components.next(), Some(Component::RootDir));
	/// assert_eq!(components.next(), Some(Component::Normal(OsStr::new("tmp"))));
	/// assert_eq!(components.next(), Some(Component::Normal(OsStr::new("foo.txt"))));
	/// assert_eq!(components.next(), None)
	/// ```
	#[inline]
	pub fn components(&self) -> std::path::Components<'_> {
		self.inner.components()
	}

	/// Produces an iterator over the path's components viewed as [`OsStr`] slices.
	///
	/// For more information about the particulars of how the path is separated into components,
	/// see [`components`](Path::components).
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	/// use std::ffi::OsStr;
	///
	/// let mut it = Path::new("/tmp/foo.txt").iter();
	/// assert_eq!(it.next(), Some(OsStr::new(&std::path::MAIN_SEPARATOR.to_string())));
	/// assert_eq!(it.next(), Some(OsStr::new("tmp")));
	/// assert_eq!(it.next(), Some(OsStr::new("foo.txt")));
	/// assert_eq!(it.next(), None)
	/// ```
	#[inline]
	pub fn iter(&self) -> std::path::Iter<'_> {
		self.inner.iter()
	}

	/// Returns an object that implements [`Display`](fmt::Display) for safely printing paths
	/// that may contain non-Unicode data. This may perform lossy conversion,
	/// depending on the platform. If you would like an implementation which escapes the path
	/// please use [`Debug`](fmt::Debug) instead.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let path = Path::new("/tmp/foo.rs");
	///
	/// println!("{}", path.display());
	/// ```
	#[inline]
	pub fn display(&self) -> std::path::Display<'_> {
		self.inner.display()
	}

	/// Converts a [`Box<Path>`](Box) into a [`PathBuf`] without copying or allocating.
	#[inline]
	pub fn into_path_buf(self: Box<Self>) -> PathBuf<Form> {
		// Safety: `Path<Form>` is a repr(transparent) wrapper around `std::path::Path`.
		let ptr = Box::into_raw(self) as *mut std::path::Path;
		let boxed = unsafe { Box::from_raw(ptr) };
		PathBuf::new_unchecked(boxed.into_path_buf())
	}

	/// Returns a reference to the same [`Path`] in a different form.
	///
	/// [`PathForm`]s can be converted to one another based on [`PathCast`] implementations.
	/// Namely, the following form conversions are possible:
	/// - [`Relative`], [`Absolute`], or [`Canonical`] into [`Any`].
	/// - [`Canonical`] into [`Absolute`].
	/// - Any form into itself.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::{Path, RelativePath};
	///
	/// let relative = RelativePath::try_new("test.txt").unwrap();
	/// let p: &Path = relative.cast();
	/// assert_eq!(p, relative);
	/// ```
	#[inline]
	pub fn cast<To>(&self) -> &Path<To>
	where
		To: PathForm,
		Form: PathCast<To>,
	{
		Path::new_unchecked(self)
	}

	/// Returns a reference to a path with its form as [`Any`].
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::{Path, RelativePath};
	///
	/// let p = RelativePath::try_new("test.txt").unwrap();
	/// assert_eq!(Path::new("test.txt"), p.as_any());
	/// ```
	#[inline]
	pub fn as_any(&self) -> &Path {
		Path::new_unchecked(self)
	}
}

impl Path {
	/// Create a new [`Path`] by wrapping a string slice.
	///
	/// This is a cost-free conversion.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// Path::new("foo.txt");
	/// ```
	///
	/// You can create [`Path`]s from [`String`]s, or even other [`Path`]s:
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let string = String::from("foo.txt");
	/// let from_string = Path::new(&string);
	/// let from_path = Path::new(&from_string);
	/// assert_eq!(from_string, from_path);
	/// ```
	#[inline]
	pub fn new<P: AsRef<OsStr> + ?Sized>(path: &P) -> &Self {
		Self::new_unchecked(path)
	}

	/// Returns a mutable reference to the underlying [`OsStr`] slice.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::{Path, PathBuf};
	///
	/// let mut path = PathBuf::from("Foo.TXT");
	///
	/// assert_ne!(path, Path::new("foo.txt"));
	///
	/// path.as_mut_os_str().make_ascii_lowercase();
	/// assert_eq!(path, Path::new("foo.txt"));
	/// ```
	#[must_use]
	#[inline]
	pub fn as_mut_os_str(&mut self) -> &mut OsStr {
		self.inner.as_mut_os_str()
	}

	/// Returns `true` if the [`Path`] is absolute, i.e., if it is independent of
	/// the current directory.
	///
	/// * On Unix, a path is absolute if it starts with the root,
	///   so [`is_absolute`](Path::is_absolute) and [`has_root`](Path::has_root) are equivalent.
	///
	/// * On Windows, a path is absolute if it has a prefix and starts with the root:
	///   `c:\windows` is absolute, while `c:temp` and `\temp` are not.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// assert!(!Path::new("foo.txt").is_absolute());
	/// ```
	#[must_use]
	#[inline]
	pub fn is_absolute(&self) -> bool {
		self.inner.is_absolute()
	}

	// Returns `true` if the [`Path`] is relative, i.e., not absolute.
	///
	/// See [`is_absolute`](Path::is_absolute)'s documentation for more details.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// assert!(Path::new("foo.txt").is_relative());
	/// ```
	#[must_use]
	#[inline]
	pub fn is_relative(&self) -> bool {
		self.inner.is_relative()
	}

	/// Returns an `Ok` [`AbsolutePath`] if the [`Path`] is absolute.
	/// Otherwise, returns an `Err` [`RelativePath`].
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// assert!(Path::new("test.txt").try_absolute().is_err());
	/// ```
	#[inline]
	pub fn try_absolute(&self) -> Result<&AbsolutePath, &RelativePath> {
		if self.is_absolute() {
			Ok(AbsolutePath::new_unchecked(&self.inner))
		} else {
			Err(RelativePath::new_unchecked(&self.inner))
		}
	}

	/// Returns an `Ok` [`RelativePath`] if the [`Path`] is relative.
	/// Otherwise, returns an `Err` [`AbsolutePath`].
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// assert!(Path::new("test.txt").try_relative().is_ok());
	/// ```
	#[inline]
	pub fn try_relative(&self) -> Result<&RelativePath, &AbsolutePath> {
		if self.is_relative() {
			Ok(RelativePath::new_unchecked(&self.inner))
		} else {
			Err(AbsolutePath::new_unchecked(&self.inner))
		}
	}
}

impl<Form: PathJoin> Path<Form> {
	/// Creates an owned [`PathBuf`] with `path` adjoined to `self`.
	///
	/// If `path` is absolute, it replaces the current path.
	///
	/// See [`PathBuf::push`] for more details on what it means to adjoin a path.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::{Path, PathBuf};
	///
	/// assert_eq!(Path::new("/etc").join("passwd"), PathBuf::from("/etc/passwd"));
	/// assert_eq!(Path::new("/etc").join("/bin/sh"), PathBuf::from("/bin/sh"));
	/// ```
	#[must_use]
	#[inline]
	pub fn join(&self, path: impl AsRef<Path>) -> PathBuf<Form::Output> {
		PathBuf::new_unchecked(self.inner.join(&path.as_ref().inner))
	}
}

impl<Form: PathSet> Path<Form> {
	/// Creates an owned [`PathBuf`] like `self` but with the given file name.
	///
	/// See [`PathBuf::set_file_name`] for more details.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::{Path, PathBuf};
	///
	/// let path = Path::new("/tmp/foo.png");
	/// assert_eq!(path.with_file_name("bar"), PathBuf::from("/tmp/bar"));
	/// assert_eq!(path.with_file_name("bar.txt"), PathBuf::from("/tmp/bar.txt"));
	///
	/// let path = Path::new("/tmp");
	/// assert_eq!(path.with_file_name("var"), PathBuf::from("/var"));
	/// ```
	#[inline]
	pub fn with_file_name(&self, file_name: impl AsRef<OsStr>) -> PathBuf<Form> {
		PathBuf::new_unchecked(self.inner.with_file_name(file_name))
	}

	/// Creates an owned [`PathBuf`] like `self` but with the given extension.
	///
	/// See [`PathBuf::set_extension`] for more details.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::{Path, PathBuf};
	///
	/// let path = Path::new("foo.rs");
	/// assert_eq!(path.with_extension("txt"), PathBuf::from("foo.txt"));
	///
	/// let path = Path::new("foo.tar.gz");
	/// assert_eq!(path.with_extension(""), PathBuf::from("foo.tar"));
	/// assert_eq!(path.with_extension("xz"), PathBuf::from("foo.tar.xz"));
	/// assert_eq!(path.with_extension("").with_extension("txt"), PathBuf::from("foo.txt"));
	/// ```
	#[inline]
	pub fn with_extension(&self, extension: impl AsRef<OsStr>) -> PathBuf<Form> {
		PathBuf::new_unchecked(self.inner.with_extension(extension))
	}
}

impl<Form: MaybeRelative> Path<Form> {
	/// Returns the, potentially relative, underlying [`std::path::Path`].
	///
	/// # Note
	///
	/// Caution should be taken when using this function. Nushell keeps track of an emulated current
	/// working directory, and using the [`std::path::Path`] returned from this method will likely
	/// use [`std::env::current_dir`] to resolve the path instead of using the emulated current
	/// working directory.
	///
	/// Instead, you should probably join this path onto the emulated current working directory.
	/// Any [`AbsolutePath`] or [`CanonicalPath`] will also suffice.
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// let p = Path::new("test.txt");
	/// assert_eq!(std::path::Path::new("test.txt"), p.as_relative_std_path());
	/// ```
	#[inline]
	pub fn as_relative_std_path(&self) -> &std::path::Path {
		&self.inner
	}

	// Returns `true` if the [`Path`] has a root.
	///
	/// * On Unix, a path has a root if it begins with `/`.
	///
	/// * On Windows, a path has a root if it:
	///     * has no prefix and begins with a separator, e.g., `\windows`
	///     * has a prefix followed by a separator, e.g., `c:\windows` but not `c:windows`
	///     * has any non-disk prefix, e.g., `\\server\share`
	///
	/// # Examples
	///
	/// ```
	/// use xeno_nu_path::Path;
	///
	/// assert!(Path::new("/etc/passwd").has_root());
	/// ```
	#[must_use]
	#[inline]
	pub fn has_root(&self) -> bool {
		self.inner.has_root()
	}
}

impl<Form: IsAbsolute> Path<Form> {
	/// Returns the underlying [`std::path::Path`].
	///
	/// # Examples
	///
	#[cfg_attr(not(windows), doc = "```")]
	#[cfg_attr(windows, doc = "```no_run")]
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let p = AbsolutePath::try_new("/test").unwrap();
	/// assert_eq!(std::path::Path::new("/test"), p.as_std_path());
	/// ```
	#[inline]
	pub fn as_std_path(&self) -> &std::path::Path {
		&self.inner
	}

	/// Converts a [`Path`] to an owned [`std::path::PathBuf`].
	///
	/// # Examples
	///
	#[cfg_attr(not(windows), doc = "```")]
	#[cfg_attr(windows, doc = "```no_run")]
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let path = AbsolutePath::try_new("/foo").unwrap();
	/// assert_eq!(path.to_std_path_buf(), std::path::PathBuf::from("/foo"));
	/// ```
	#[inline]
	pub fn to_std_path_buf(&self) -> std::path::PathBuf {
		self.inner.to_path_buf()
	}

	/// Queries the file system to get information about a file, directory, etc.
	///
	/// This function will traverse symbolic links to query information about the destination file.
	///
	/// This is an alias to [`std::fs::metadata`].
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let path = AbsolutePath::try_new("/Minas/tirith").unwrap();
	/// let metadata = path.metadata().expect("metadata call failed");
	/// println!("{:?}", metadata.file_type());
	/// ```
	#[inline]
	pub fn metadata(&self) -> io::Result<fs::Metadata> {
		self.inner.metadata()
	}

	/// Returns an iterator over the entries within a directory.
	///
	/// The iterator will yield instances of <code>[io::Result]<[fs::DirEntry]></code>.
	/// New errors may be encountered after an iterator is initially constructed.
	///
	/// This is an alias to [`std::fs::read_dir`].
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let path = AbsolutePath::try_new("/laputa").unwrap();
	/// for entry in path.read_dir().expect("read_dir call failed") {
	///     if let Ok(entry) = entry {
	///         println!("{:?}", entry.path());
	///     }
	/// }
	/// ```
	#[inline]
	pub fn read_dir(&self) -> io::Result<fs::ReadDir> {
		self.inner.read_dir()
	}

	/// Returns `true` if the path points at an existing entity.
	///
	/// Warning: this method may be error-prone, consider using [`try_exists`](Path::try_exists)
	/// instead! It also has a risk of introducing time-of-check to time-of-use (TOCTOU) bugs.
	///
	/// This function will traverse symbolic links to query information about the destination file.
	///
	/// If you cannot access the metadata of the file, e.g. because of a permission error
	/// or broken symbolic links, this will return `false`.
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let path = AbsolutePath::try_new("/does_not_exist").unwrap();
	/// assert!(!path.exists());
	/// ```
	#[must_use]
	#[inline]
	pub fn exists(&self) -> bool {
		self.inner.exists()
	}

	/// Returns `true` if the path exists on disk and is pointing at a regular file.
	///
	/// This function will traverse symbolic links to query information about the destination file.
	///
	/// If you cannot access the metadata of the file, e.g. because of a permission error
	/// or broken symbolic links, this will return `false`.
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let path = AbsolutePath::try_new("/is_a_directory/").unwrap();
	/// assert_eq!(path.is_file(), false);
	///
	/// let path = AbsolutePath::try_new("/a_file.txt").unwrap();
	/// assert_eq!(path.is_file(), true);
	/// ```
	///
	/// # See Also
	///
	/// When the goal is simply to read from (or write to) the source, the most reliable way
	/// to test the source can be read (or written to) is to open it. Only using `is_file` can
	/// break workflows like `diff <( prog_a )` on a Unix-like system for example.
	/// See [`std::fs::File::open`] or [`std::fs::OpenOptions::open`] for more information.
	#[must_use]
	#[inline]
	pub fn is_file(&self) -> bool {
		self.inner.is_file()
	}

	/// Returns `true` if the path exists on disk and is pointing at a directory.
	///
	/// This function will traverse symbolic links to query information about the destination file.
	///
	/// If you cannot access the metadata of the file, e.g. because of a permission error
	/// or broken symbolic links, this will return `false`.
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let path = AbsolutePath::try_new("/is_a_directory/").unwrap();
	/// assert_eq!(path.is_dir(), true);
	///
	/// let path = AbsolutePath::try_new("/a_file.txt").unwrap();
	/// assert_eq!(path.is_dir(), false);
	/// ```
	#[must_use]
	#[inline]
	pub fn is_dir(&self) -> bool {
		self.inner.is_dir()
	}
}

impl AbsolutePath {
	/// Returns the canonical, absolute form of the path with all intermediate components
	/// normalized and symbolic links resolved.
	///
	/// On Windows, this will also simplify to a winuser path.
	///
	/// This is an alias to [`std::fs::canonicalize`].
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::{AbsolutePath, PathBuf};
	///
	/// let path = AbsolutePath::try_new("/foo/test/../test/bar.rs").unwrap();
	/// assert_eq!(path.canonicalize().unwrap(), PathBuf::from("/foo/test/bar.rs"));
	/// ```
	#[cfg(not(windows))]
	#[inline]
	pub fn canonicalize(&self) -> io::Result<CanonicalPathBuf> {
		self.inner.canonicalize().map(CanonicalPathBuf::new_unchecked)
	}

	/// Returns the canonical, absolute form of the path with all intermediate components
	/// normalized and symbolic links resolved.
	///
	/// On Windows, this will also simplify to a winuser path.
	///
	/// This is an alias to [`std::fs::canonicalize`].
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::{AbsolutePath, PathBuf};
	///
	/// let path = AbsolutePath::try_new("/foo/test/../test/bar.rs").unwrap();
	/// assert_eq!(path.canonicalize().unwrap(), PathBuf::from("/foo/test/bar.rs"));
	/// ```
	#[cfg(windows)]
	pub fn canonicalize(&self) -> io::Result<CanonicalPathBuf> {
		use omnipath::WinPathExt;

		let path = self.inner.canonicalize()?.to_winuser_path()?;
		Ok(CanonicalPathBuf::new_unchecked(path))
	}

	/// Reads a symbolic link, returning the file that the link points to.
	///
	/// This is an alias to [`std::fs::read_link`].
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let path = AbsolutePath::try_new("/laputa/sky_castle.rs").unwrap();
	/// let path_link = path.read_link().expect("read_link call failed");
	/// ```
	#[inline]
	pub fn read_link(&self) -> io::Result<AbsolutePathBuf> {
		self.inner.read_link().map(PathBuf::new_unchecked)
	}

	/// Returns `Ok(true)` if the path points at an existing entity.
	///
	/// This function will traverse symbolic links to query information about the destination file.
	/// In case of broken symbolic links this will return `Ok(false)`.
	///
	/// [`Path::exists`] only checks whether or not a path was both found and readable.
	/// By contrast, [`try_exists`](Path::try_exists) will return `Ok(true)` or `Ok(false)`,
	/// respectively, if the path was _verified_ to exist or not exist.
	/// If its existence can neither be confirmed nor denied, it will propagate an `Err` instead.
	/// This can be the case if e.g. listing permission is denied on one of the parent directories.
	///
	/// Note that while this avoids some pitfalls of the [`exists`](Path::exists) method,
	/// it still can not prevent time-of-check to time-of-use (TOCTOU) bugs.
	/// You should only use it in scenarios where those bugs are not an issue.
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let path = AbsolutePath::try_new("/does_not_exist").unwrap();
	/// assert!(!path.try_exists().unwrap());
	///
	/// let path = AbsolutePath::try_new("/root/secret_file.txt").unwrap();
	/// assert!(path.try_exists().is_err());
	/// ```
	#[inline]
	pub fn try_exists(&self) -> io::Result<bool> {
		self.inner.try_exists()
	}

	/// Returns `true` if the path exists on disk and is pointing at a symbolic link.
	///
	/// This function will not traverse symbolic links.
	/// In case of a broken symbolic link this will also return true.
	///
	/// If you cannot access the directory containing the file, e.g., because of a permission error,
	/// this will return false.
	///
	/// # Examples
	///
	#[cfg_attr(unix, doc = "```no_run")]
	#[cfg_attr(not(unix), doc = "```ignore")]
	/// use xeno_nu_path::AbsolutePath;
	/// use std::os::unix::fs::symlink;
	///
	/// let link_path = AbsolutePath::try_new("/link").unwrap();
	/// symlink("/origin_does_not_exist/", link_path).unwrap();
	/// assert_eq!(link_path.is_symlink(), true);
	/// assert_eq!(link_path.exists(), false);
	/// ```
	#[must_use]
	#[inline]
	pub fn is_symlink(&self) -> bool {
		self.inner.is_symlink()
	}

	/// Queries the metadata about a file without following symlinks.
	///
	/// This is an alias to [`std::fs::symlink_metadata`].
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let path = AbsolutePath::try_new("/Minas/tirith").unwrap();
	/// let metadata = path.symlink_metadata().expect("symlink_metadata call failed");
	/// println!("{:?}", metadata.file_type());
	/// ```
	#[inline]
	pub fn symlink_metadata(&self) -> io::Result<fs::Metadata> {
		self.inner.symlink_metadata()
	}
}

impl CanonicalPath {
	/// Returns a [`CanonicalPath`] as a [`AbsolutePath`].
	///
	/// # Examples
	///
	/// ```no_run
	/// use xeno_nu_path::AbsolutePath;
	///
	/// let absolute = AbsolutePath::try_new("/test").unwrap();
	/// let p = absolute.canonicalize().unwrap();
	/// assert_eq!(absolute, p.as_absolute());
	/// ```
	#[inline]
	pub fn as_absolute(&self) -> &AbsolutePath {
		self.cast()
	}
}

impl<Form: PathForm> fmt::Debug for Path<Form> {
	fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(&self.inner, fmt)
	}
}

impl<Form: PathForm> Clone for Box<Path<Form>> {
	#[inline]
	fn clone(&self) -> Self {
		std_box_to_box(self.inner.into())
	}
}

impl<Form: PathForm> ToOwned for Path<Form> {
	type Owned = PathBuf<Form>;

	#[inline]
	fn to_owned(&self) -> Self::Owned {
		self.to_path_buf()
	}

	#[inline]
	fn clone_into(&self, target: &mut PathBuf<Form>) {
		self.inner.clone_into(&mut target.inner);
	}
}

impl<'a, Form: PathForm> IntoIterator for &'a Path<Form> {
	type Item = &'a OsStr;

	type IntoIter = std::path::Iter<'a>;

	#[inline]
	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

/// An iterator over [`Path`] and its ancestors.
///
/// This `struct` is created by the [`ancestors`](Path::ancestors) method on [`Path`].
/// See its documentation for more.
///
/// # Examples
///
/// ```
/// use xeno_nu_path::Path;
///
/// let path = Path::new("/foo/bar");
///
/// for ancestor in path.ancestors() {
///     println!("{}", ancestor.display());
/// }
/// ```
#[derive(Clone, Copy)]
pub struct Ancestors<'a, Form: PathForm> {
	_form: PhantomData<Form>,
	inner: std::path::Ancestors<'a>,
}

impl<Form: PathForm> fmt::Debug for Ancestors<'_, Form> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Debug::fmt(&self.inner, f)
	}
}

impl<'a, Form: PathForm> Iterator for Ancestors<'a, Form> {
	type Item = &'a Path<Form>;

	fn next(&mut self) -> Option<Self::Item> {
		self.inner.next().map(Path::new_unchecked)
	}
}

impl<Form: PathForm> FusedIterator for Ancestors<'_, Form> {}
