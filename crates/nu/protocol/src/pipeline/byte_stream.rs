//! Module managing the streaming of raw bytes between pipeline elements
//!
//! This module also handles conversions the [`ShellError`] <-> [`io::Error`](std::io::Error),
//! so remember the usage of [`ShellErrorBridge`] where applicable.
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, ErrorKind, Read, Write};
use std::ops::Bound;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use xeno_nu_utils::SplitRead as SplitReadInner;

use crate::shell_error::bridge::ShellErrorBridge;
use crate::shell_error::io::IoError;
use crate::{IntRange, PipelineData, ShellError, Signals, Span, Type, Value};

include!("byte_stream_source.rs");
include!("byte_stream_stream.rs");
include!("byte_stream_readers.rs");

#[cfg(test)]
mod tests {
	use super::*;

	fn test_chunks<T>(data: Vec<T>, type_: ByteStreamType) -> Chunks
	where
		T: AsRef<[u8]> + Default + Send + 'static,
	{
		let reader = ReadIterator {
			iter: data.into_iter(),
			cursor: Some(Cursor::new(T::default())),
		};
		Chunks::new(SourceReader::Read(Box::new(reader)), Span::test_data(), Signals::empty(), type_)
	}

	#[test]
	fn chunks_read_binary_passthrough() {
		let bins = vec![&[0, 1][..], &[2, 3][..]];
		let iter = test_chunks(bins.clone(), ByteStreamType::Binary);

		let bins_values: Vec<Value> = bins.into_iter().map(|bin| Value::binary(bin, Span::test_data())).collect();
		assert_eq!(bins_values, iter.collect::<Result<Vec<Value>, _>>().expect("error"));
	}

	#[test]
	fn chunks_read_string_clean() {
		let strs = vec!["Nushell", "が好きです"];
		let iter = test_chunks(strs.clone(), ByteStreamType::String);

		let strs_values: Vec<Value> = strs.into_iter().map(|string| Value::string(string, Span::test_data())).collect();
		assert_eq!(strs_values, iter.collect::<Result<Vec<Value>, _>>().expect("error"));
	}

	#[test]
	fn chunks_read_string_split_boundary() {
		let real = "Nushell最高!";
		let chunks = vec![&b"Nushell\xe6"[..], &b"\x9c\x80\xe9"[..], &b"\xab\x98!"[..]];
		let iter = test_chunks(chunks.clone(), ByteStreamType::String);

		let mut string = String::new();
		for value in iter {
			let chunk_string = value.expect("error").into_string().expect("not a string");
			string.push_str(&chunk_string);
		}
		assert_eq!(real, string);
	}

	#[test]
	fn chunks_read_string_utf8_error() {
		let chunks = vec![&b"Nushell\xe6"[..], &b"\x9c\x80\xe9"[..], &b"\xab"[..]];
		let iter = test_chunks(chunks, ByteStreamType::String);

		let mut string = String::new();
		for value in iter {
			match value {
				Ok(value) => string.push_str(&value.into_string().expect("not a string")),
				Err(err) => {
					println!("string so far: {string:?}");
					println!("got error: {err:?}");
					assert!(!string.is_empty());
					assert!(matches!(err, ShellError::NonUtf8Custom { .. }));
					return;
				}
			}
		}
		panic!("no error");
	}

	#[test]
	fn chunks_read_unknown_fallback() {
		let chunks = vec![&b"Nushell"[..], &b"\x9c\x80\xe9abcd"[..], &b"efgh"[..]];
		let mut iter = test_chunks(chunks, ByteStreamType::Unknown);

		let mut get = || iter.next().expect("end of iter").expect("error");

		assert_eq!(Value::test_string("Nushell"), get());
		assert_eq!(Value::test_binary(b"\x9c\x80\xe9abcd"), get());
		// Once it's in binary mode it won't go back
		assert_eq!(Value::test_binary(b"efgh"), get());
	}
}
