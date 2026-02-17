#![cfg_attr(test, allow(unused_crate_dependencies))]
//! Shared file presentation helpers.
//!
//! Centralizes file icon resolution and label formatting so statusline,
//! completions, and other UI surfaces stay visually consistent.

use std::path::Path;

use devicons::FileIcon;

/// Generic plain-file fallback icon used when the icon database has no match.
pub const GENERIC_FILE_ICON: &str = "󰈔";
/// Generic directory icon used when callers know the item is a directory.
pub const DIRECTORY_ICON: &str = "󰉋";

/// Semantic kind of file-system entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileKind {
	#[default]
	File,
	Directory,
}

/// Label formatting mode for file entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileDisplayMode {
	/// Preserve the path text exactly as provided by the caller.
	#[default]
	AsProvided,
	/// Display only the last path segment.
	FileName,
	/// Display a path relative to `working_dir` when possible.
	RelativeToWorkingDir,
	/// Display an absolute path when possible.
	Absolute,
}

/// Rendering context for file-label formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileDisplayContext<'a> {
	pub mode: FileDisplayMode,
	pub working_dir: Option<&'a Path>,
}

impl Default for FileDisplayContext<'_> {
	fn default() -> Self {
		Self {
			mode: FileDisplayMode::AsProvided,
			working_dir: None,
		}
	}
}

/// Input item for file presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileItem<'a> {
	pub path: &'a Path,
	pub label_override: Option<&'a str>,
	pub kind: FileKind,
}

impl<'a> FileItem<'a> {
	pub fn new(path: &'a Path) -> Self {
		Self {
			path,
			label_override: None,
			kind: FileKind::File,
		}
	}

	pub fn with_label_override(mut self, label: &'a str) -> Self {
		self.label_override = Some(label);
		self
	}

	pub fn with_kind(mut self, kind: FileKind) -> Self {
		self.kind = kind;
		self
	}
}

/// Resolved icon + label payload for UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePresentation {
	icon: String,
	label: String,
}

impl FilePresentation {
	pub fn new(icon: String, label: String) -> Self {
		Self { icon, label }
	}

	pub fn icon(&self) -> &str {
		&self.icon
	}

	pub fn label(&self) -> &str {
		&self.label
	}
}

/// Resolves icon + label in one call for a file item.
pub fn present_file(item: FileItem<'_>, context: FileDisplayContext<'_>) -> FilePresentation {
	let icon = file_icon_for_path(item.path, item.kind);
	let label = format_file_label(item.path, item.label_override, context);
	FilePresentation::new(icon, label)
}

/// Resolves the icon glyph for a file path.
pub fn file_icon_for_path(path: &Path, kind: FileKind) -> String {
	match kind {
		FileKind::Directory => DIRECTORY_ICON.to_string(),
		FileKind::File => {
			let icon = FileIcon::from(path).icon;
			if icon == '*' { GENERIC_FILE_ICON.to_string() } else { icon.to_string() }
		}
	}
}

/// Formats a path label according to the selected display mode.
pub fn format_file_label(path: &Path, label_override: Option<&str>, context: FileDisplayContext<'_>) -> String {
	match context.mode {
		FileDisplayMode::AsProvided => label_override.map(std::borrow::ToOwned::to_owned).unwrap_or_else(|| path.display().to_string()),
		FileDisplayMode::FileName => path
			.file_name()
			.map(|name| name.to_string_lossy().to_string())
			.or_else(|| label_override.map(std::borrow::ToOwned::to_owned))
			.unwrap_or_else(|| path.display().to_string()),
		FileDisplayMode::RelativeToWorkingDir => {
			if path.is_absolute() {
				if let Some(working_dir) = context.working_dir
					&& let Ok(rel) = path.strip_prefix(working_dir)
				{
					return rel.display().to_string();
				}
				path.display().to_string()
			} else {
				path.display().to_string()
			}
		}
		FileDisplayMode::Absolute => {
			if path.is_absolute() {
				path.display().to_string()
			} else if let Some(working_dir) = context.working_dir {
				working_dir.join(path).display().to_string()
			} else {
				std::env::current_dir()
					.map(|cwd| cwd.join(path).display().to_string())
					.unwrap_or_else(|_| path.display().to_string())
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn file_icon_uses_generic_fallback_for_unknown_extension() {
		let icon = file_icon_for_path(Path::new("notes.some_unknown_ext_xeno"), FileKind::File);
		assert_eq!(icon, GENERIC_FILE_ICON);
	}

	#[test]
	fn file_icon_uses_directory_icon_for_directory_kind() {
		let icon = file_icon_for_path(Path::new("src"), FileKind::Directory);
		assert_eq!(icon, DIRECTORY_ICON);
	}

	#[test]
	fn file_icon_uses_devicon_for_known_filetypes() {
		let icon = file_icon_for_path(Path::new("Cargo.toml"), FileKind::File);
		assert_ne!(icon, GENERIC_FILE_ICON);
		assert_ne!(icon, "*");
	}

	#[test]
	fn format_file_label_uses_override_for_as_provided() {
		let label = format_file_label(
			Path::new("/tmp/real-name.txt"),
			Some("../alias-name.txt"),
			FileDisplayContext {
				mode: FileDisplayMode::AsProvided,
				working_dir: None,
			},
		);
		assert_eq!(label, "../alias-name.txt");
	}

	#[test]
	fn format_file_label_can_render_relative_to_working_dir() {
		let label = format_file_label(
			Path::new("/tmp/xeno/src/main.rs"),
			None,
			FileDisplayContext {
				mode: FileDisplayMode::RelativeToWorkingDir,
				working_dir: Some(Path::new("/tmp/xeno")),
			},
		);
		assert_eq!(label, "src/main.rs");
	}

	#[test]
	fn present_file_returns_icon_and_label() {
		let item = FileItem::new(Path::new("Cargo.toml")).with_label_override("Cargo.toml");
		let presentation = present_file(item, FileDisplayContext::default());
		assert_eq!(presentation.label(), "Cargo.toml");
		assert!(!presentation.icon().is_empty());
	}
}
