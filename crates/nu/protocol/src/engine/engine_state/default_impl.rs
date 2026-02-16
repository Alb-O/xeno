impl Default for EngineState {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod engine_state_tests {
	use std::str::{Utf8Error, from_utf8};

	use super::*;
	use crate::engine::StateWorkingSet;

	#[test]
	fn add_file_gives_id() {
		let engine_state = EngineState::new();
		let mut engine_state = StateWorkingSet::new(&engine_state);
		let id = engine_state.add_file("test.nu".into(), &[]);

		assert_eq!(id, FileId::new(0));
	}

	#[test]
	fn add_file_gives_id_including_parent() {
		let mut engine_state = EngineState::new();
		let parent_id = engine_state.add_file("test.nu".into(), Arc::new([]));

		let mut working_set = StateWorkingSet::new(&engine_state);
		let working_set_id = working_set.add_file("child.nu".into(), &[]);

		assert_eq!(parent_id, FileId::new(0));
		assert_eq!(working_set_id, FileId::new(1));
	}

	#[test]
	fn merge_states() -> Result<(), ShellError> {
		let mut engine_state = EngineState::new();
		engine_state.add_file("test.nu".into(), Arc::new([]));

		let delta = {
			let mut working_set = StateWorkingSet::new(&engine_state);
			let _ = working_set.add_file("child.nu".into(), &[]);
			working_set.render()
		};

		engine_state.merge_delta(delta)?;

		assert_eq!(engine_state.num_files(), 2);
		assert_eq!(&*engine_state.files[0].name, "test.nu");
		assert_eq!(&*engine_state.files[1].name, "child.nu");

		Ok(())
	}

	#[test]
	fn list_variables() -> Result<(), Utf8Error> {
		let varname = "something";
		let varname_with_sigil = "$".to_owned() + varname;
		let engine_state = EngineState::new();
		let mut working_set = StateWorkingSet::new(&engine_state);
		working_set.add_variable(varname.as_bytes().into(), Span { start: 0, end: 1 }, Type::Int, false);
		let variables = working_set
			.list_variables()
			.into_iter()
			.map(from_utf8)
			.collect::<Result<Vec<&str>, Utf8Error>>()?;
		assert_eq!(variables, vec![varname_with_sigil]);
		Ok(())
	}

	#[test]
	fn get_plugin_config() {
		let mut engine_state = EngineState::new();

		assert!(engine_state.get_plugin_config("example").is_none(), "Unexpected plugin configuration");

		let mut plugins = HashMap::new();
		plugins.insert("example".into(), Value::string("value", Span::test_data()));

		let mut config = Config::clone(engine_state.get_config());
		config.plugins = plugins;

		engine_state.set_config(config);

		assert!(engine_state.get_plugin_config("example").is_some(), "Plugin configuration not found");
	}
}

#[cfg(test)]
mod test_cwd {
	//! Here're the test cases we need to cover:
	//!
	//! `EngineState::cwd()` computes the result from `self.env_vars["PWD"]` and
	//! optionally `stack.env_vars["PWD"]`.
	//!
	//! PWD may be unset in either `env_vars`.
	//! PWD should NOT be an empty string.
	//! PWD should NOT be a non-string value.
	//! PWD should NOT be a relative path.
	//! PWD should NOT contain trailing slashes.
	//! PWD may point to a directory or a symlink to directory.
	//! PWD should NOT point to a file or a symlink to file.
	//! PWD should NOT point to non-existent entities in the filesystem.

	use tempfile::{NamedTempFile, TempDir};
	use xeno_nu_path::{AbsolutePath, Path, assert_path_eq};

	use crate::Value;
	use crate::engine::{EngineState, Stack};

	/// Creates a symlink. Works on both Unix and Windows.
	#[cfg(any(unix, windows))]
	fn symlink(original: impl AsRef<AbsolutePath>, link: impl AsRef<AbsolutePath>) -> std::io::Result<()> {
		let original = original.as_ref();
		let link = link.as_ref();

		#[cfg(unix)]
		{
			std::os::unix::fs::symlink(original, link)
		}
		#[cfg(windows)]
		{
			if original.is_dir() {
				std::os::windows::fs::symlink_dir(original, link)
			} else {
				std::os::windows::fs::symlink_file(original, link)
			}
		}
	}

	/// Create an engine state initialized with the given PWD.
	fn engine_state_with_pwd(path: impl AsRef<Path>) -> EngineState {
		let mut engine_state = EngineState::new();
		engine_state.add_env_var("PWD".into(), Value::test_string(path.as_ref().to_str().unwrap()));
		engine_state
	}

	/// Create a stack initialized with the given PWD.
	fn stack_with_pwd(path: impl AsRef<Path>) -> Stack {
		let mut stack = Stack::new();
		stack.add_env_var("PWD".into(), Value::test_string(path.as_ref().to_str().unwrap()));
		stack
	}

	#[test]
	fn pwd_not_set() {
		let engine_state = EngineState::new();
		engine_state.cwd(None).unwrap_err();
	}

	#[test]
	fn pwd_is_empty_string() {
		let engine_state = engine_state_with_pwd("");
		engine_state.cwd(None).unwrap_err();
	}

	#[test]
	fn pwd_is_non_string_value() {
		let mut engine_state = EngineState::new();
		engine_state.add_env_var("PWD".into(), Value::test_glob("*"));
		engine_state.cwd(None).unwrap_err();
	}

	#[test]
	fn pwd_is_relative_path() {
		let engine_state = engine_state_with_pwd("./foo");

		engine_state.cwd(None).unwrap_err();
	}

	#[test]
	fn pwd_has_trailing_slash() {
		let dir = TempDir::new().unwrap();
		let engine_state = engine_state_with_pwd(dir.path().join(""));

		engine_state.cwd(None).unwrap_err();
	}

	#[test]
	fn pwd_points_to_root() {
		#[cfg(windows)]
		let root = Path::new(r"C:\");
		#[cfg(not(windows))]
		let root = Path::new("/");

		let engine_state = engine_state_with_pwd(root);
		let cwd = engine_state.cwd(None).unwrap();
		assert_path_eq!(cwd, root);
	}

	#[test]
	fn pwd_points_to_normal_file() {
		let file = NamedTempFile::new().unwrap();
		let engine_state = engine_state_with_pwd(file.path());

		engine_state.cwd(None).unwrap_err();
	}

	#[test]
	fn pwd_points_to_normal_directory() {
		let dir = TempDir::new().unwrap();
		let engine_state = engine_state_with_pwd(dir.path());

		let cwd = engine_state.cwd(None).unwrap();
		assert_path_eq!(cwd, dir.path());
	}

	#[test]
	fn pwd_points_to_symlink_to_file() {
		let file = NamedTempFile::new().unwrap();
		let temp_file = AbsolutePath::try_new(file.path()).unwrap();
		let dir = TempDir::new().unwrap();
		let temp = AbsolutePath::try_new(dir.path()).unwrap();

		let link = temp.join("link");
		symlink(temp_file, &link).unwrap();
		let engine_state = engine_state_with_pwd(&link);

		engine_state.cwd(None).unwrap_err();
	}

	#[test]
	fn pwd_points_to_symlink_to_directory() {
		let dir = TempDir::new().unwrap();
		let temp = AbsolutePath::try_new(dir.path()).unwrap();

		let link = temp.join("link");
		symlink(temp, &link).unwrap();
		let engine_state = engine_state_with_pwd(&link);

		let cwd = engine_state.cwd(None).unwrap();
		assert_path_eq!(cwd, link);
	}

	#[test]
	fn pwd_points_to_broken_symlink() {
		let dir = TempDir::new().unwrap();
		let temp = AbsolutePath::try_new(dir.path()).unwrap();
		let other_dir = TempDir::new().unwrap();
		let other_temp = AbsolutePath::try_new(other_dir.path()).unwrap();

		let link = temp.join("link");
		symlink(other_temp, &link).unwrap();
		let engine_state = engine_state_with_pwd(&link);

		drop(other_dir);
		engine_state.cwd(None).unwrap_err();
	}

	#[test]
	fn pwd_points_to_nonexistent_entity() {
		let engine_state = engine_state_with_pwd(TempDir::new().unwrap().path());

		engine_state.cwd(None).unwrap_err();
	}

	#[test]
	fn stack_pwd_not_set() {
		let dir = TempDir::new().unwrap();
		let engine_state = engine_state_with_pwd(dir.path());
		let stack = Stack::new();

		let cwd = engine_state.cwd(Some(&stack)).unwrap();
		assert_eq!(cwd, dir.path());
	}

	#[test]
	fn stack_pwd_is_empty_string() {
		let dir = TempDir::new().unwrap();
		let engine_state = engine_state_with_pwd(dir.path());
		let stack = stack_with_pwd("");

		engine_state.cwd(Some(&stack)).unwrap_err();
	}

	#[test]
	fn stack_pwd_points_to_normal_directory() {
		let dir1 = TempDir::new().unwrap();
		let dir2 = TempDir::new().unwrap();
		let engine_state = engine_state_with_pwd(dir1.path());
		let stack = stack_with_pwd(dir2.path());

		let cwd = engine_state.cwd(Some(&stack)).unwrap();
		assert_path_eq!(cwd, dir2.path());
	}

	#[test]
	fn stack_pwd_points_to_normal_directory_with_symlink_components() {
		let dir = TempDir::new().unwrap();
		let temp = AbsolutePath::try_new(dir.path()).unwrap();

		// `/tmp/dir/link` points to `/tmp/dir`, then we set PWD to `/tmp/dir/link/foo`
		let link = temp.join("link");
		symlink(temp, &link).unwrap();
		let foo = link.join("foo");
		std::fs::create_dir(temp.join("foo")).unwrap();
		let engine_state = EngineState::new();
		let stack = stack_with_pwd(&foo);

		let cwd = engine_state.cwd(Some(&stack)).unwrap();
		assert_path_eq!(cwd, foo);
	}
}
