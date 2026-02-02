use core::fmt;
use std::fmt::Display;
use std::net::AddrParseError;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

use heed3::byteorder::BE;
use heed3::types::{Bytes, U128};
use heed3::{Database, Error as HeedError, MdbError, PutFlags, RwTxn};
use serde::{Deserialize, Serialize};
use sonic_rs::Error as SonicError;
use thiserror::Error;

use crate::helix_engine::reranker::errors::RerankerError;
use crate::helixc::parser::errors::ParserError;
use crate::helixc::parser::types::{Field, FieldPrefix};
use crate::protocol::value_error::ValueError;

#[derive(Debug, Error)]
pub enum StorageError {
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Storage connection error: {msg} {source}")]
	Connection {
		msg: String,
		#[source]
		source: std::io::Error,
	},

	#[error("Storage error: {0}")]
	Backend(String),

	#[error("Conversion error: {0}")]
	Conversion(String),

	#[error("Decode error: {0}")]
	Decode(String),

	#[error("Duplicate key on unique index: {0}")]
	DuplicateKey(String),

	#[error("Config file not found")]
	ConfigFileNotFound,

	#[error("Slice length error")]
	SliceLengthError,
}

impl From<HeedError> for StorageError {
	fn from(error: HeedError) -> Self {
		match error {
			HeedError::Mdb(MdbError::KeyExist) => StorageError::DuplicateKey(error.to_string()),
			_ => StorageError::Backend(error.to_string()),
		}
	}
}

impl From<AddrParseError> for StorageError {
	fn from(error: AddrParseError) -> Self {
		StorageError::Conversion(format!("AddrParseError: {error}"))
	}
}

impl From<SonicError> for StorageError {
	fn from(error: SonicError) -> Self {
		StorageError::Conversion(format!("sonic error: {error}"))
	}
}

impl From<FromUtf8Error> for StorageError {
	fn from(error: FromUtf8Error) -> Self {
		StorageError::Conversion(format!("FromUtf8Error: {error}"))
	}
}

impl From<postcard::Error> for StorageError {
	fn from(error: postcard::Error) -> Self {
		StorageError::Conversion(format!("postcard error: {error}"))
	}
}

impl From<ParserError> for StorageError {
	fn from(error: ParserError) -> Self {
		StorageError::Conversion(format!("ParserError: {error}"))
	}
}

impl From<Utf8Error> for StorageError {
	fn from(error: Utf8Error) -> Self {
		StorageError::Conversion(format!("Utf8Error: {error}"))
	}
}

impl From<uuid::Error> for StorageError {
	fn from(error: uuid::Error) -> Self {
		StorageError::Conversion(format!("uuid error: {error}"))
	}
}

#[derive(Debug, Error)]
pub enum TraversalError {
	#[error("Traversal error: {0}")]
	Message(String),

	#[error("Edge not found")]
	EdgeNotFound,

	#[error("Node not found")]
	NodeNotFound,

	#[error("Label not found")]
	LabelNotFound,

	#[error("Shortest path not found")]
	ShortestPathNotFound,

	#[error("Multiple nodes with same id")]
	MultipleNodesWithSameId,

	#[error("Multiple edges with same id")]
	MultipleEdgesWithSameId,

	#[error("Invalid node")]
	InvalidNode,

	#[error("Parameter {0} not found in request")]
	ParamNotFound(&'static str),

	#[error("Unsupported value type")]
	UnsupportedValueType,
}

#[derive(Debug, Error)]
pub enum EmbeddingError {
	#[error("Error while embedding text: {0}")]
	Message(String),
}

#[derive(Debug, Error)]
pub enum EngineError {
	#[error(transparent)]
	Storage(#[from] StorageError),

	#[error(transparent)]
	Traversal(#[from] TraversalError),

	#[error(transparent)]
	Vector(#[from] VectorError),

	#[error(transparent)]
	Value(#[from] ValueError),

	#[error(transparent)]
	Reranker(#[from] RerankerError),

	#[error(transparent)]
	Embedding(#[from] EmbeddingError),
}

impl From<HeedError> for EngineError {
	fn from(error: HeedError) -> Self {
		StorageError::from(error).into()
	}
}

impl From<std::io::Error> for EngineError {
	fn from(error: std::io::Error) -> Self {
		StorageError::Io(error).into()
	}
}

impl From<AddrParseError> for EngineError {
	fn from(error: AddrParseError) -> Self {
		StorageError::from(error).into()
	}
}

impl From<SonicError> for EngineError {
	fn from(error: SonicError) -> Self {
		StorageError::from(error).into()
	}
}

impl From<FromUtf8Error> for EngineError {
	fn from(error: FromUtf8Error) -> Self {
		StorageError::from(error).into()
	}
}

impl From<postcard::Error> for EngineError {
	fn from(error: postcard::Error) -> Self {
		StorageError::from(error).into()
	}
}

impl From<ParserError> for EngineError {
	fn from(error: ParserError) -> Self {
		StorageError::from(error).into()
	}
}

impl From<Utf8Error> for EngineError {
	fn from(error: Utf8Error) -> Self {
		StorageError::from(error).into()
	}
}

impl From<uuid::Error> for EngineError {
	fn from(error: uuid::Error) -> Self {
		StorageError::from(error).into()
	}
}

#[derive(Debug, Error)]
pub enum VectorError {
	#[error("Vector not found: {0}")]
	VectorNotFound(String),
	#[error("Vector deleted")]
	VectorDeleted,
	#[error("Invalid vector length")]
	InvalidVectorLength,
	#[error("Invalid vector data")]
	InvalidVectorData,
	#[error("Entry point not found")]
	EntryPointNotFound,
	#[error("Conversion error: {0}")]
	ConversionError(String),
	#[error("Vector core error: {0}")]
	VectorCoreError(String),
	#[error("Vector already deleted: {0}")]
	VectorAlreadyDeleted(String),
}

impl From<HeedError> for VectorError {
	fn from(error: HeedError) -> Self {
		VectorError::VectorCoreError(format!("heed error: {error}"))
	}
}

impl From<FromUtf8Error> for VectorError {
	fn from(error: FromUtf8Error) -> Self {
		VectorError::ConversionError(format!("FromUtf8Error: {error}"))
	}
}

impl From<Utf8Error> for VectorError {
	fn from(error: Utf8Error) -> Self {
		VectorError::ConversionError(format!("Utf8Error: {error}"))
	}
}

impl From<SonicError> for VectorError {
	fn from(error: SonicError) -> Self {
		VectorError::ConversionError(format!("SonicError: {error}"))
	}
}

impl From<postcard::Error> for VectorError {
	fn from(error: postcard::Error) -> Self {
		VectorError::ConversionError(format!("postcard error: {error}"))
	}
}

#[cfg(test)]
mod tests {
	use std::error::Error as _;
	use std::io;

	use super::{ActiveSecondaryIndex, EngineError, SecondaryIndex, StorageError, TraversalError};

	#[test]
	fn test_secondary_index_into_active() {
		let unique = SecondaryIndex::Unique("email".to_string());
		let active = unique.into_active();
		assert!(matches!(active, Some(ActiveSecondaryIndex::Unique(ref n)) if n == "email"));

		let index = SecondaryIndex::Index("age".to_string());
		let active = index.into_active();
		assert!(matches!(active, Some(ActiveSecondaryIndex::Index(ref n)) if n == "age"));

		let none = SecondaryIndex::None;
		assert!(none.into_active().is_none());
	}

	#[test]
	fn test_active_secondary_index_display() {
		let unique = ActiveSecondaryIndex::Unique("email".to_string());
		assert_eq!(
			unique.to_string(),
			"ActiveSecondaryIndex::Unique(\"email\")"
		);

		let index = ActiveSecondaryIndex::Index("age".to_string());
		assert_eq!(index.to_string(), "ActiveSecondaryIndex::Index(\"age\")");
	}

	#[test]
	fn test_engine_error_roundtrip_display() {
		let err = EngineError::from(StorageError::Conversion("bad".to_string()));
		let msg = err.to_string();
		assert!(msg.contains("Conversion error"));
	}

	#[test]
	fn test_engine_error_sources_chain() {
		let io_err = io::Error::new(io::ErrorKind::Other, "disk");
		let err = EngineError::from(StorageError::Io(io_err));
		let source = err.source().expect("expected source");
		assert!(source.to_string().contains("disk"));
		let err = EngineError::from(TraversalError::Message("fail".to_string()));
		assert!(err.source().is_none());
	}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SecondaryIndex {
	Unique(String),
	Index(String),
	None,
}

impl Display for SecondaryIndex {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Unique(name) => write!(f, "SecondaryIndex::Unique(\"{name}\".to_string())"),
			Self::Index(name) => write!(f, "SecondaryIndex::Index(\"{name}\".to_string())"),
			SecondaryIndex::None => write!(f, ""),
		}
	}
}

impl SecondaryIndex {
	pub fn from_field(field: &Field) -> Self {
		match field.prefix {
			FieldPrefix::Index => Self::Index(field.name.clone()),
			FieldPrefix::UniqueIndex => Self::Unique(field.name.clone()),
			FieldPrefix::Optional | FieldPrefix::Empty => Self::None,
		}
	}

	/// Converts to `ActiveSecondaryIndex`, returning `None` for `SecondaryIndex::None`.
	pub fn into_active(self) -> Option<ActiveSecondaryIndex> {
		match self {
			Self::Unique(name) => Some(ActiveSecondaryIndex::Unique(name)),
			Self::Index(name) => Some(ActiveSecondaryIndex::Index(name)),
			Self::None => None,
		}
	}
}

/// A secondary index that is guaranteed to be active (either `Unique` or `Index`).
///
/// Unlike `SecondaryIndex`, this enum has no `None` variant, so callers never need
/// to handle a logically impossible case. Use `SecondaryIndex::into_active()` to
/// convert from the schema-level enum, which filters out `None` at the boundary.
#[derive(Debug, Clone)]
pub enum ActiveSecondaryIndex {
	Unique(String),
	Index(String),
}

impl Display for ActiveSecondaryIndex {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Unique(name) => write!(f, "ActiveSecondaryIndex::Unique(\"{name}\")"),
			Self::Index(name) => write!(f, "ActiveSecondaryIndex::Index(\"{name}\")"),
		}
	}
}

impl ActiveSecondaryIndex {
	/// Inserts a key→id mapping into the secondary index database.
	///
	/// For `Unique` indices, uses `NO_OVERWRITE` and allows idempotent re-insertion
	/// of the same id. For `Index` indices, performs a plain `put`.
	pub fn insert(
		&self,
		db: &Database<Bytes, U128<BE>>,
		txn: &mut RwTxn,
		key: &[u8],
		id: &u128,
	) -> Result<(), EngineError> {
		match self {
			Self::Unique(name) => {
				if let Err(e) = db.put_with_flags(txn, PutFlags::NO_OVERWRITE, key, id) {
					match e {
						HeedError::Mdb(MdbError::KeyExist) => {
							if let Some(existing_id) =
								db.get(txn, key).map_err(StorageError::from)?
								&& &existing_id == id
							{
								return Ok(());
							}
							return Err(StorageError::DuplicateKey(format!(
								"Duplicate key for unique index {name}"
							))
							.into());
						}
						_ => return Err(StorageError::from(e).into()),
					}
				}
				Ok(())
			}
			Self::Index(_) => {
				db.put(txn, key, id).map_err(StorageError::from)?;
				Ok(())
			}
		}
	}

	/// Deletes a key→id mapping from the secondary index database.
	///
	/// For `Unique` indices, deletes only if the stored id matches. For `Index`
	/// indices, deletes the specific duplicate entry.
	pub fn delete(
		&self,
		db: &Database<Bytes, U128<BE>>,
		txn: &mut RwTxn,
		key: &[u8],
		id: &u128,
	) -> Result<(), EngineError> {
		match self {
			Self::Unique(_) => {
				if let Some(existing_id) = db.get(txn, key).map_err(StorageError::from)?
					&& &existing_id == id
				{
					db.delete(txn, key).map_err(StorageError::from)?;
				}
				Ok(())
			}
			Self::Index(_) => {
				db.delete_one_duplicate(txn, key, id)
					.map_err(StorageError::from)?;
				Ok(())
			}
		}
	}
}
