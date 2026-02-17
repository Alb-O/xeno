#![cfg_attr(test, allow(unused_crate_dependencies))]
//! Shared file presentation helpers.
//!
//! Centralizes file icon resolution and label formatting so statusline,
//! completions, and other UI surfaces stay visually consistent.
//!
//! Includes virtual/scratch buffer identity helpers so non-file buffers can
//! share the same icon + label presentation pipeline.

use std::path::Path;

use devicons::FileIcon;

/// Generic plain-file fallback icon used when the icon database has no match.
pub const GENERIC_FILE_ICON: &str = "󰈔";
/// Generic directory icon used when callers know the item is a directory.
pub const DIRECTORY_ICON: &str = "󰉋";
/// Generic scratch-buffer icon.
pub const SCRATCH_ICON: &str = GENERIC_FILE_ICON;
/// Command palette virtual-buffer icon.
pub const COMMAND_PALETTE_ICON: &str = "󰘳";
/// File picker virtual-buffer icon.
pub const FILE_PICKER_ICON: &str = "󰈙";
/// Search virtual-buffer icon.
pub const SEARCH_ICON: &str = "󰍉";
/// Rename virtual-buffer icon.
pub const RENAME_ICON: &str = "󰑕";
/// Workspace search virtual-buffer icon.
pub const WORKSPACE_SEARCH_ICON: &str = "󰍉";
/// Overlay list-pane virtual-buffer icon.
pub const OVERLAY_LIST_ICON: &str = "󰅩";
/// Overlay preview-pane virtual-buffer icon.
pub const OVERLAY_PREVIEW_ICON: &str = "󰈈";
/// Generic overlay virtual-buffer icon.
pub const OVERLAY_ICON: &str = "󰏌";

/// Semantic kind of file-system entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileKind {
	#[default]
	File,
	Directory,
}

/// Semantic identity for non-file virtual buffers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualBufferKind {
	CommandPalette,
	FilePicker,
	Search,
	Rename,
	WorkspaceSearch,
	OverlayList,
	OverlayPreview,
	OverlayCustom(String),
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

/// Rendering context for buffer-label formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BufferDisplayContext<'a> {
	pub file: FileDisplayContext<'a>,
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

/// Semantic identity input for unified buffer presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferIdentity<'a> {
	File { path: &'a Path, kind: FileKind },
	Scratch,
	Virtual(VirtualBufferKind),
}

/// Input item for buffer presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferItem<'a> {
	pub identity: BufferIdentity<'a>,
	pub label_override: Option<&'a str>,
}

impl<'a> BufferItem<'a> {
	pub fn file(path: &'a Path) -> Self {
		Self {
			identity: BufferIdentity::File { path, kind: FileKind::File },
			label_override: None,
		}
	}

	pub fn scratch() -> Self {
		Self {
			identity: BufferIdentity::Scratch,
			label_override: None,
		}
	}

	pub fn virtual_buffer(kind: VirtualBufferKind) -> Self {
		Self {
			identity: BufferIdentity::Virtual(kind),
			label_override: None,
		}
	}

	pub fn with_file_kind(mut self, kind: FileKind) -> Self {
		if let BufferIdentity::File { kind: file_kind, .. } = &mut self.identity {
			*file_kind = kind;
		}
		self
	}

	pub fn with_label_override(mut self, label: &'a str) -> Self {
		self.label_override = Some(label);
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

/// Resolved icon + label payload for buffer UI surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferPresentation {
	icon: String,
	label: String,
}

impl BufferPresentation {
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

/// Resolves icon + label in one call for any buffer identity.
pub fn present_buffer(item: BufferItem<'_>, context: BufferDisplayContext<'_>) -> BufferPresentation {
	match item.identity {
		BufferIdentity::File { path, kind } => {
			let mut file = FileItem::new(path).with_kind(kind);
			if let Some(label_override) = item.label_override {
				file = file.with_label_override(label_override);
			}
			let presentation = present_file(file, context.file);
			BufferPresentation::new(presentation.icon().to_string(), presentation.label().to_string())
		}
		BufferIdentity::Scratch => BufferPresentation::new(
			SCRATCH_ICON.to_string(),
			item.label_override
				.map(std::borrow::ToOwned::to_owned)
				.unwrap_or_else(|| "[scratch]".to_string()),
		),
		BufferIdentity::Virtual(kind) => {
			let (icon, label) = virtual_identity(kind, item.label_override);
			BufferPresentation::new(icon, label)
		}
	}
}

fn virtual_identity(kind: VirtualBufferKind, label_override: Option<&str>) -> (String, String) {
	match kind {
		VirtualBufferKind::CommandPalette => (COMMAND_PALETTE_ICON.to_string(), "[Command Palette]".to_string()),
		VirtualBufferKind::FilePicker => (FILE_PICKER_ICON.to_string(), "[File Picker]".to_string()),
		VirtualBufferKind::Search => (SEARCH_ICON.to_string(), "[Search]".to_string()),
		VirtualBufferKind::Rename => (RENAME_ICON.to_string(), "[Rename]".to_string()),
		VirtualBufferKind::WorkspaceSearch => (WORKSPACE_SEARCH_ICON.to_string(), "[Workspace Search]".to_string()),
		VirtualBufferKind::OverlayList => (
			OVERLAY_LIST_ICON.to_string(),
			label_override.map(|label| format!("[{label} List]")).unwrap_or_else(|| "[List]".to_string()),
		),
		VirtualBufferKind::OverlayPreview => (
			OVERLAY_PREVIEW_ICON.to_string(),
			label_override
				.map(|label| format!("[{label} Preview]"))
				.unwrap_or_else(|| "[Preview]".to_string()),
		),
		VirtualBufferKind::OverlayCustom(name) => (
			OVERLAY_ICON.to_string(),
			label_override
				.map(std::borrow::ToOwned::to_owned)
				.unwrap_or_else(|| format!("[Overlay: {name}]")),
		),
	}
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

	#[test]
	fn present_buffer_file_returns_icon_and_label() {
		let presentation = present_buffer(
			BufferItem::file(Path::new("Cargo.toml")).with_label_override("Cargo.toml"),
			BufferDisplayContext::default(),
		);
		assert_eq!(presentation.label(), "Cargo.toml");
		assert!(!presentation.icon().is_empty());
	}

	#[test]
	fn present_buffer_scratch_uses_named_label() {
		let presentation = present_buffer(BufferItem::scratch(), BufferDisplayContext::default());
		assert_eq!(presentation.label(), "[scratch]");
		assert_eq!(presentation.icon(), SCRATCH_ICON);
	}

	#[test]
	fn present_buffer_virtual_command_palette_uses_named_identity() {
		let presentation = present_buffer(BufferItem::virtual_buffer(VirtualBufferKind::CommandPalette), BufferDisplayContext::default());
		assert_eq!(presentation.label(), "[Command Palette]");
		assert_eq!(presentation.icon(), COMMAND_PALETTE_ICON);
	}

	#[test]
	fn present_buffer_virtual_custom_uses_overlay_fallback() {
		let presentation = present_buffer(
			BufferItem::virtual_buffer(VirtualBufferKind::OverlayCustom("MyOverlay".to_string())),
			BufferDisplayContext::default(),
		);
		assert_eq!(presentation.label(), "[Overlay: MyOverlay]");
		assert_eq!(presentation.icon(), OVERLAY_ICON);
	}

	#[test]
	fn present_buffer_virtual_list_uses_title_hint() {
		let presentation = present_buffer(
			BufferItem::virtual_buffer(VirtualBufferKind::OverlayList).with_label_override("Workspace Search"),
			BufferDisplayContext::default(),
		);
		assert_eq!(presentation.label(), "[Workspace Search List]");
		assert_eq!(presentation.icon(), OVERLAY_LIST_ICON);
	}
}
