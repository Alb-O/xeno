//! Token storage utilities.
//!
//! Handles persistent storage of OAuth tokens using XDG base directories.
//! Auth tokens are stored in XDG_DATA_HOME/xeno/auth.json (~/.local/share/xeno/auth.json).

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::error::AuthError;
use crate::error::AuthResult;
use crate::token::AuthState;

/// Application directory name.
const APP_DIR: &str = "xeno";

/// Auth state filename.
const AUTH_FILE: &str = "auth.json";

/// Get the default Xeno data directory following XDG spec.
///
/// Returns XDG_DATA_HOME/xeno (~/.local/share/xeno on Linux).
pub fn default_data_dir() -> AuthResult<PathBuf> {
    let data_dir = dirs::data_dir().ok_or_else(|| {
        AuthError::Storage("could not determine XDG data directory".into())
    })?;
    Ok(data_dir.join(APP_DIR))
}

/// Get the default Xeno config directory following XDG spec.
///
/// Returns XDG_CONFIG_HOME/xeno (~/.config/xeno on Linux).
pub fn default_config_dir() -> AuthResult<PathBuf> {
    let config_dir = dirs::config_dir().ok_or_else(|| {
        AuthError::Storage("could not determine XDG config directory".into())
    })?;
    Ok(config_dir.join(APP_DIR))
}

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

    let contents = fs::read_to_string(&path).map_err(|e| {
        AuthError::Storage(format!("failed to read {}: {e}", path.display()))
    })?;

    let state: AuthState = serde_json::from_str(&contents).map_err(|e| {
        AuthError::Storage(format!("failed to parse {}: {e}", path.display()))
    })?;

    Ok(Some(state))
}

/// Save authentication state to disk.
pub fn save_auth(data_dir: &Path, state: &AuthState) -> AuthResult<()> {
    fs::create_dir_all(data_dir).map_err(|e| {
        AuthError::Storage(format!(
            "failed to create {}: {e}",
            data_dir.display()
        ))
    })?;

    let path = auth_file_path(data_dir);
    let contents = serde_json::to_string_pretty(state).map_err(|e| {
        AuthError::Storage(format!("failed to serialize auth state: {e}"))
    })?;

    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, &contents).map_err(|e| {
        AuthError::Storage(format!("failed to write {}: {e}", temp_path.display()))
    })?;

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

/// Delete authentication state from disk.
pub fn delete_auth(data_dir: &Path) -> AuthResult<bool> {
    let path = auth_file_path(data_dir);

    if !path.exists() {
        return Ok(false);
    }

    fs::remove_file(&path).map_err(|e| {
        AuthError::Storage(format!("failed to delete {}: {e}", path.display()))
    })?;

    Ok(true)
}

/// Login with an API key (stores to disk).
pub fn login_with_api_key(data_dir: &Path, api_key: &str) -> AuthResult<()> {
    let state = AuthState::from_api_key(api_key.to_string());
    save_auth(data_dir, &state)
}

/// Logout by deleting stored credentials.
pub fn logout(data_dir: &Path) -> AuthResult<bool> {
    delete_auth(data_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn save_and_load_api_key() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        login_with_api_key(data_dir, "sk-test-key").unwrap();

        let loaded = load_auth(data_dir).unwrap().unwrap();
        assert_eq!(loaded.api_key.as_deref(), Some("sk-test-key"));
        assert!(loaded.tokens.is_none());
    }

    #[test]
    fn load_missing_returns_none() {
        let temp = TempDir::new().unwrap();
        let loaded = load_auth(temp.path()).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn logout_removes_file() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        login_with_api_key(data_dir, "sk-test").unwrap();
        assert!(auth_file_path(data_dir).exists());

        let removed = logout(data_dir).unwrap();
        assert!(removed);
        assert!(!auth_file_path(data_dir).exists());
    }

    #[test]
    fn logout_missing_returns_false() {
        let temp = TempDir::new().unwrap();
        let removed = logout(temp.path()).unwrap();
        assert!(!removed);
    }

    #[cfg(unix)]
    #[test]
    fn auth_file_has_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let data_dir = temp.path();

        login_with_api_key(data_dir, "sk-test").unwrap();

        let metadata = fs::metadata(auth_file_path(data_dir)).unwrap();
        let mode = metadata.permissions().mode();

        assert_eq!(mode & 0o777, 0o600);
    }
}
