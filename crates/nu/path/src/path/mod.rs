use std::borrow::{Borrow, Cow};
use std::cmp::Ordering;
use std::collections::TryReserveError;
use std::convert::Infallible;
use std::ffi::{OsStr, OsString};
use std::hash::{Hash, Hasher};
use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::path::StripPrefixError;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use std::{fmt, fs, io};

use ref_cast::{RefCastCustom, ref_cast_custom};

use crate::form::{Absolute, Any, Canonical, IsAbsolute, MaybeRelative, PathCast, PathForm, PathJoin, PathPush, PathSet, Relative};

/// A wrapper around [`std::path::Path`] with extra invariants determined by its `Form`.
///
/// The possible path forms are [`Any`], [`Relative`], [`Absolute`], or [`Canonical`].
/// To learn more, view the documentation on [`PathForm`] or any of the individual forms.
///
/// There are also several type aliases available, corresponding to each [`PathForm`]:
/// - [`RelativePath`] (same as [`Path<Relative>`])
/// - [`AbsolutePath`] (same as [`Path<Absolute>`])
/// - [`CanonicalPath`] (same as [`Path<Canonical>`])
///
/// If the `Form` is not specified, then it defaults to [`Any`], so [`Path`] and [`Path<Any>`]
/// are one in the same.
///
/// # Converting to [`std::path`] types
///
/// [`Path`]s with form [`Any`] cannot be easily referenced as a [`std::path::Path`] by design.
/// Other Nushell crates need to account for the emulated current working directory
/// before passing a path to functions in [`std`] or other third party crates.
/// You can [`join`](Path::join) a [`Path`] onto an [`AbsolutePath`] or a [`CanonicalPath`].
/// This will return an [`AbsolutePathBuf`] which can be easily referenced as a [`std::path::Path`].
/// If you really mean it, you can instead use [`as_relative_std_path`](Path::as_relative_std_path)
/// to get the underlying [`std::path::Path`] from a [`Path`].
/// But this may cause third-party code to use [`std::env::current_dir`] to resolve
/// the path which is almost always incorrect behavior. Extra care is needed to ensure that this
/// is not the case after using [`as_relative_std_path`](Path::as_relative_std_path).
#[derive(RefCastCustom)]
#[repr(transparent)]
pub struct Path<Form = Any> {
	_form: PhantomData<Form>,
	inner: std::path::Path,
}

/// A path that is strictly relative.
///
/// I.e., this path is guaranteed to never be absolute.
///
/// [`RelativePath`]s cannot be easily converted into a [`std::path::Path`] by design.
/// Other Nushell crates need to account for the emulated current working directory
/// before passing a path to functions in [`std`] or other third party crates.
/// You can [`join`](Path::join) a [`RelativePath`] onto an [`AbsolutePath`] or a [`CanonicalPath`].
/// This will return an [`AbsolutePathBuf`] which can be referenced as a [`std::path::Path`].
/// If you really mean it, you can use [`as_relative_std_path`](RelativePath::as_relative_std_path)
/// to get the underlying [`std::path::Path`] from a [`RelativePath`].
/// But this may cause third-party code to use [`std::env::current_dir`] to resolve
/// the path which is almost always incorrect behavior. Extra care is needed to ensure that this
/// is not the case after using [`as_relative_std_path`](RelativePath::as_relative_std_path).
///
/// # Examples
///
/// [`RelativePath`]s can be created by using [`try_relative`](Path::try_relative)
/// on a [`Path`], by using [`try_new`](Path::try_new), or by using
/// [`strip_prefix`](Path::strip_prefix) on a [`Path`] of any form.
///
/// ```
/// use xeno_nu_path::{Path, RelativePath};
///
/// let path1 = Path::new("foo.txt");
/// let path1 = path1.try_relative().unwrap();
///
/// let path2 = RelativePath::try_new("foo.txt").unwrap();
///
/// let path3 = Path::new("/prefix/foo.txt").strip_prefix("/prefix").unwrap();
///
/// assert_eq!(path1, path2);
/// assert_eq!(path2, path3);
/// ```
///
/// You can also use `RelativePath::try_from` or `try_into`.
/// This supports attempted conversions from [`Path`] as well as types in [`std::path`].
///
/// ```
/// use xeno_nu_path::{Path, RelativePath};
///
/// let path1 = Path::new("foo.txt");
/// let path1: &RelativePath = path1.try_into().unwrap();
///
/// let path2 = std::path::Path::new("foo.txt");
/// let path2: &RelativePath = path2.try_into().unwrap();
///
/// assert_eq!(path1, path2)
/// ```
pub type RelativePath = Path<Relative>;

/// A path that is strictly absolute.
///
/// I.e., this path is guaranteed to never be relative.
///
/// # Examples
///
/// [`AbsolutePath`]s can be created by using [`try_absolute`](Path::try_absolute) on a [`Path`]
/// or by using [`try_new`](AbsolutePath::try_new).
///
#[cfg_attr(not(windows), doc = "```")]
#[cfg_attr(windows, doc = "```no_run")]
/// use xeno_nu_path::{AbsolutePath, Path};
///
/// let path1 = Path::new("/foo").try_absolute().unwrap();
/// let path2 = AbsolutePath::try_new("/foo").unwrap();
///
/// assert_eq!(path1, path2);
/// ```
///
/// You can also use `AbsolutePath::try_from` or `try_into`.
/// This supports attempted conversions from [`Path`] as well as types in [`std::path`].
///
#[cfg_attr(not(windows), doc = "```")]
#[cfg_attr(windows, doc = "```no_run")]
/// use xeno_nu_path::{AbsolutePath, Path};
///
/// let path1 = Path::new("/foo");
/// let path1: &AbsolutePath = path1.try_into().unwrap();
///
/// let path2 = std::path::Path::new("/foo");
/// let path2: &AbsolutePath = path2.try_into().unwrap();
///
/// assert_eq!(path1, path2)
/// ```
pub type AbsolutePath = Path<Absolute>;

/// An absolute, canonical path.
///
/// # Examples
///
/// [`CanonicalPath`]s can only be created by using [`canonicalize`](Path::canonicalize) on
/// an [`AbsolutePath`]. References to [`CanonicalPath`]s can be converted to
/// [`AbsolutePath`] references using `as_ref`, [`cast`](Path::cast),
/// or [`as_absolute`](CanonicalPath::as_absolute).
///
/// ```no_run
/// use xeno_nu_path::AbsolutePath;
///
/// let path = AbsolutePath::try_new("/foo").unwrap();
///
/// let canonical = path.canonicalize().expect("canonicalization failed");
///
/// assert_eq!(path, canonical.as_absolute());
/// ```
pub type CanonicalPath = Path<Canonical>;

include!("path_impl.rs");
include!("path_buf.rs");
