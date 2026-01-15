//! Codex token storage utilities.
//!
//! Handles persistent storage of Codex OAuth tokens.

use std::fs;
use std::path::{Path, PathBuf};

use super::token::AuthState;
use crate::error::{AuthError, AuthResult};

/// Auth state filename.
const AUTH_FILE: &str = "auth.json";

/// Get the path to the auth file within a data directory.
pub fn auth_file_path(data_dir: &Path) -> PathBuf {
	data_dir.join(AUTH_FILE)
}

/// Load authentication state from disk.
pub fn load_auth(data_dir: &Path) -> AuthResult<Option<AuthState>> {
	let path = auth_file_path(data_dir);

	if !path.exists() {
		return Ok(None);
	}

	let contents = fs::read_to_string(&path)
		.map_err(|e| AuthError::Storage(format!("failed to read {}: {e}", path.display())))?;

	let state: AuthState = serde_json::from_str(&contents)
		.map_err(|e| AuthError::Storage(format!("failed to parse {}: {e}", path.display())))?;

	Ok(Some(state))
}

/// Save authentication state to disk.
pub fn save_auth(data_dir: &Path, state: &AuthState) -> AuthResult<()> {
	fs::create_dir_all(data_dir)
		.map_err(|e| AuthError::Storage(format!("failed to create {}: {e}", data_dir.display())))?;

	let path = auth_file_path(data_dir);
	let contents = serde_json::to_string_pretty(state)
		.map_err(|e| AuthError::Storage(format!("failed to serialize auth state: {e}")))?;

	let temp_path = path.with_extension("json.tmp");
	fs::write(&temp_path, &contents)
		.map_err(|e| AuthError::Storage(format!("failed to write {}: {e}", temp_path.display())))?;

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		let perms = fs::Permissions::from_mode(0o600);
		fs::set_permissions(&temp_path, perms).map_err(|e| {
			AuthError::Storage(format!(
				"failed to set permissions on {}: {e}",
				temp_path.display()
			))
		})?;
	}

	fs::rename(&temp_path, &path).map_err(|e| {
		AuthError::Storage(format!(
			"failed to rename {} to {}: {e}",
			temp_path.display(),
			path.display()
		))
	})?;

	Ok(())
}

/// Logout by deleting stored credentials.
pub fn logout(data_dir: &Path) -> AuthResult<bool> {
	let path = auth_file_path(data_dir);

	if !path.exists() {
		return Ok(false);
	}

	fs::remove_file(&path)
		.map_err(|e| AuthError::Storage(format!("failed to delete {}: {e}", path.display())))?;

	Ok(true)
}

#[cfg(test)]
mod tests {
	use tempfile::TempDir;

	use super::*;

	#[test]
	fn save_and_load_api_key() {
		let temp = TempDir::new().unwrap();
		let state = AuthState::from_api_key("sk-test-key".into());
		save_auth(temp.path(), &state).unwrap();

		let loaded = load_auth(temp.path()).unwrap().unwrap();
		assert_eq!(loaded.api_key.as_deref(), Some("sk-test-key"));
		assert!(loaded.tokens.is_none());
	}

	#[test]
	fn load_missing_returns_none() {
		let temp = TempDir::new().unwrap();
		assert!(load_auth(temp.path()).unwrap().is_none());
	}

	#[test]
	fn logout_removes_file() {
		let temp = TempDir::new().unwrap();
		let state = AuthState::from_api_key("sk-test".into());
		save_auth(temp.path(), &state).unwrap();
		assert!(auth_file_path(temp.path()).exists());

		assert!(logout(temp.path()).unwrap());
		assert!(!auth_file_path(temp.path()).exists());
	}

	#[test]
	fn logout_missing_returns_false() {
		let temp = TempDir::new().unwrap();
		assert!(!logout(temp.path()).unwrap());
	}

	#[cfg(unix)]
	#[test]
	fn auth_file_has_restricted_permissions() {
		use std::os::unix::fs::PermissionsExt;

		let temp = TempDir::new().unwrap();
		let state = AuthState::from_api_key("sk-test".into());
		save_auth(temp.path(), &state).unwrap();

		let metadata = fs::metadata(auth_file_path(temp.path())).unwrap();
		assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
	}
}
