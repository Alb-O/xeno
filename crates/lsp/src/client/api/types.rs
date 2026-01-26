use lsp_types::{Uri, WorkspaceFolder};

/// Create a workspace folder from a URI.
pub fn workspace_folder_from_uri(uri: Uri) -> WorkspaceFolder {
	let name = uri
		.as_str()
		.rsplit('/')
		.next()
		.filter(|s| !s.is_empty())
		.unwrap_or_default()
		.to_string();
	WorkspaceFolder { name, uri }
}
