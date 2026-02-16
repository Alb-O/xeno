//! Command specification schema.
//!
//! Defines declarative command metadata entries for registry ingestion.

#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaletteCommitPolicy {
	#[default]
	AllowPartial,
	RequireResolvedArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteArgKind {
	FilePath,
	ThemeName,
	SnippetRefOrBody,
	OptionKey,
	OptionValue,
	BufferRef,
	CommandName,
	FreeText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteArgSpec {
	pub name: String,
	pub kind: PaletteArgKind,
	#[serde(default)]
	pub required: bool,
	#[serde(default)]
	pub variadic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommandPaletteSpec {
	#[serde(default)]
	pub args: Vec<PaletteArgSpec>,
	#[serde(default)]
	pub commit_policy: PaletteCommitPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
	pub common: MetaCommonSpec,
	pub palette: CommandPaletteSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandsSpec {
	#[serde(default)]
	pub commands: Vec<CommandSpec>,
}
