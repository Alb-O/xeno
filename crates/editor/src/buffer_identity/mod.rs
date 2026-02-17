//! Unified view-identity and presentation resolution.
//!
//! Centralizes classification of editor views into file, scratch, or virtual
//! overlay identities, then maps that identity into a display-ready icon/label
//! payload consumed by statusline and document title surfaces.

use std::path::PathBuf;

use crate::Editor;
use crate::buffer::ViewId;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResolvedBufferIdentity {
	File(PathBuf),
	Scratch,
	Virtual(crate::overlay::VirtualBufferIdentity),
}

impl Editor {
	fn resolve_buffer_identity(&self, view_id: ViewId) -> ResolvedBufferIdentity {
		let Some(buffer) = self.get_buffer(view_id) else {
			return ResolvedBufferIdentity::Scratch;
		};

		if let Some(path) = buffer.path() {
			return ResolvedBufferIdentity::File(path);
		}

		if let Some(identity) = self.virtual_buffer_identity(view_id) {
			return ResolvedBufferIdentity::Virtual(identity);
		}

		ResolvedBufferIdentity::Scratch
	}

	/// Returns virtual identity metadata for an overlay pane buffer.
	pub fn virtual_buffer_identity(&self, view_id: ViewId) -> Option<crate::overlay::VirtualBufferIdentity> {
		let active = self.state.overlay_system.interaction().active()?;
		active.session.virtual_identity_for_buffer(view_id).cloned()
	}

	/// Resolves icon + label presentation for a view buffer.
	pub fn buffer_presentation(&self, view_id: ViewId) -> xeno_file_display::BufferPresentation {
		let context = xeno_file_display::BufferDisplayContext::default();

		match self.resolve_buffer_identity(view_id) {
			ResolvedBufferIdentity::File(path) => xeno_file_display::present_buffer(xeno_file_display::BufferItem::file(path.as_path()), context),
			ResolvedBufferIdentity::Virtual(identity) => {
				let mut item = xeno_file_display::BufferItem::virtual_buffer(identity.kind);
				if let Some(title_hint) = identity.title_hint.as_deref() {
					item = item.with_label_override(title_hint);
				}
				xeno_file_display::present_buffer(item, context)
			}
			ResolvedBufferIdentity::Scratch => xeno_file_display::present_buffer(xeno_file_display::BufferItem::scratch(), context),
		}
	}
}
