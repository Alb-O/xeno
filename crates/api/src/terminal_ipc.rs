//! IPC infrastructure for embedded terminal command integration.
//!
//! Provides a Unix socket-based mechanism allowing shell commands in embedded
//! terminals to invoke editor commands. When a terminal spawns:
//!
//! 1. A temp directory with shell script wrappers (e.g., `:write`, `:quit`) is created
//! 2. That directory is prepended to `$PATH`
//! 3. `$TOME_SOCKET` points to our Unix socket
//!
//! The wrappers send tab-separated messages over the socket, which the editor
//! polls and dispatches to command handlers.
//!
//! # Architecture
//!
//! Split into two parts for thread-safety:
//! - [`TerminalIpcEnv`] - Shareable paths (`Send + Sync`), held by terminals
//! - [`TerminalIpc`] - Owned by Editor, contains the receiver for polling

use std::collections::VecDeque;
use std::fs::{self, Permissions};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;

static IPC_COUNTER: AtomicU64 = AtomicU64::new(0);

use tome_manifest::COMMANDS;

/// A request received from a shell command wrapper.
#[derive(Debug, Clone)]
pub struct IpcRequest {
	/// The command name (e.g., "write", "quit").
	pub command: String,
	/// Arguments passed to the command.
	pub args: Vec<String>,
}

/// Shareable environment configuration for terminal IPC.
///
/// Contains only the paths needed to configure spawned terminals.
/// Use [`TerminalIpc::env`] to obtain an `Arc<TerminalIpcEnv>`.
#[derive(Debug)]
pub struct TerminalIpcEnv {
	bin_dir: PathBuf,
	socket_path: PathBuf,
	fish_config_dir: Option<PathBuf>,
}

impl TerminalIpcEnv {
	/// Returns the directory containing command wrappers (prepended to `$PATH`).
	pub fn bin_dir(&self) -> &Path {
		&self.bin_dir
	}

	/// Returns the socket path for `$TOME_SOCKET`.
	pub fn socket_path(&self) -> &Path {
		&self.socket_path
	}

	pub fn fish_config_dir(&self) -> Option<&Path> {
		self.fish_config_dir.as_deref()
	}

	/// Returns environment variables to set for spawned terminals.
	pub fn env_vars(&self) -> Vec<(String, String)> {
		let current_path = std::env::var("PATH").unwrap_or_default();
		let mut vars = vec![
			(
				"PATH".to_string(),
				format!("{}:{}", self.bin_dir.display(), current_path),
			),
			(
				"TOME_SOCKET".to_string(),
				self.socket_path.display().to_string(),
			),
			// TOME_BIN allows shells that reset PATH (like fish) to add it manually
			("TOME_BIN".to_string(), self.bin_dir.display().to_string()),
		];
		if let Some(fish_dir) = &self.fish_config_dir {
			vars.push((
				"FISH_CONFIG_DIR".to_string(),
				fish_dir.display().to_string(),
			));
		}
		vars
	}
}

/// Manages IPC infrastructure for a single editor instance.
///
/// Creates a temp directory with command wrapper scripts and a Unix socket
/// for receiving requests. The Editor should own this and call [`Self::poll`]
/// regularly to process incoming commands.
pub struct TerminalIpc {
	env: Arc<TerminalIpcEnv>,
	receiver: Receiver<IpcRequest>,
	pending: VecDeque<IpcRequest>,
}

impl TerminalIpc {
	/// Creates new IPC infrastructure.
	///
	/// # Errors
	///
	/// Returns an error if the temp directory or socket cannot be created.
	pub fn new() -> std::io::Result<Self> {
		let id = IPC_COUNTER.fetch_add(1, Ordering::Relaxed);
		let base_dir = std::env::temp_dir().join(format!("tome-{}-{}", std::process::id(), id));
		let bin_dir = base_dir.join("bin");
		let socket_path = base_dir.join("socket");

		fs::create_dir_all(&bin_dir)?;
		generate_command_wrappers(&bin_dir)?;
		generate_shell_wrappers(&bin_dir, &socket_path)?;
		let fish_config_dir = generate_fish_config(&base_dir, &bin_dir, &socket_path)?;

		let _ = fs::remove_file(&socket_path);
		let listener = UnixListener::bind(&socket_path)?;
		listener.set_nonblocking(true)?;

		let (tx, rx) = channel();
		thread::spawn(move || run_listener(listener, tx));

		Ok(Self {
			env: Arc::new(TerminalIpcEnv {
				bin_dir,
				socket_path,
				fish_config_dir,
			}),
			receiver: rx,
			pending: VecDeque::new(),
		})
	}

	/// Returns a shareable reference to the environment configuration.
	pub fn env(&self) -> Arc<TerminalIpcEnv> {
		Arc::clone(&self.env)
	}

	/// Polls for pending IPC requests.
	///
	/// Returns `Some(request)` if available. Call regularly from the event loop.
	pub fn poll(&mut self) -> Option<IpcRequest> {
		loop {
			match self.receiver.try_recv() {
				Ok(req) => self.pending.push_back(req),
				Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
			}
		}
		self.pending.pop_front()
	}
}

impl Drop for TerminalIpc {
	fn drop(&mut self) {
		if let Some(parent) = self.env.bin_dir.parent() {
			let _ = fs::remove_dir_all(parent);
		}
	}
}

fn generate_command_wrappers(bin_dir: &Path) -> std::io::Result<()> {
	for cmd in COMMANDS.iter() {
		let script = generate_wrapper_script(cmd.name);
		let path = bin_dir.join(format!(":{}", cmd.name));
		fs::write(&path, &script)?;
		fs::set_permissions(&path, Permissions::from_mode(0o755))?;

		for alias in cmd.aliases {
			let alias_path = bin_dir.join(format!(":{}", alias));
			fs::write(&alias_path, generate_wrapper_script(alias))?;
			fs::set_permissions(&alias_path, Permissions::from_mode(0o755))?;
		}
	}
	Ok(())
}

fn generate_wrapper_script(cmd_name: &str) -> String {
	format!(
		r#"#!/bin/sh
[ -z "$TOME_SOCKET" ] && echo "Not in Tome editor" >&2 && exit 1
[ ! -S "$TOME_SOCKET" ] && echo "Socket not found" >&2 && exit 1
MSG="{cmd_name}"
for arg in "$@"; do MSG="$MSG	$arg"; done
printf '%s\n' "$MSG" | nc -UN "$TOME_SOCKET" 2>/dev/null && exit 0
printf '%s\n' "$MSG" | nc -U -q0 "$TOME_SOCKET" 2>/dev/null && exit 0
printf '%s\n' "$MSG" | nc -U -w1 "$TOME_SOCKET" 2>/dev/null && exit 0
"#
	)
}

fn generate_shell_wrappers(bin_dir: &Path, socket_path: &Path) -> std::io::Result<()> {
	let Some(fish_bin) = find_in_path("fish", Some(bin_dir)) else {
		return Ok(());
	};

	let script = generate_fish_wrapper_script(&fish_bin, bin_dir, socket_path);
	let path = bin_dir.join("fish");
	fs::write(&path, &script)?;
	fs::set_permissions(&path, Permissions::from_mode(0o755))?;
	Ok(())
}

fn generate_fish_wrapper_script(fish_bin: &Path, bin_dir: &Path, socket_path: &Path) -> String {
	let init = fish_init_command(bin_dir, socket_path);
	let init = escape_single_quotes(&init);
	format!(
		r#"#!/bin/sh
TOME_BIN="{bin_dir}"
export TOME_BIN
TOME_SOCKET="{socket}"
export TOME_SOCKET
exec "{fish_bin}" -i --init-command '{init}' "$@"
"#,
		bin_dir = bin_dir.display(),
		socket = socket_path.display(),
		fish_bin = fish_bin.display(),
		init = init,
	)
}

fn escape_single_quotes(value: &str) -> String {
	value.replace('\'', "'\"'\"'")
}

fn generate_fish_config(
	base_dir: &Path,
	bin_dir: &Path,
	socket_path: &Path,
) -> std::io::Result<Option<PathBuf>> {
	let fish_dir = base_dir.join("fish");
	fs::create_dir_all(&fish_dir)?;

	let mut script = String::new();
	if let Some(original) = find_original_fish_config() {
		script.push_str(&format!(
			"set -l __tome_orig_config \"{}\"\nif test -f \"$__tome_orig_config\"\n    source \"$__tome_orig_config\"\nend\n",
			original.display()
		));
	}
	script.push_str(&format!(
		"function __tome_path_refresh --on-event fish_prompt\n    if not contains -- {bin_dir} $PATH\n        set -gx PATH {bin_dir} $PATH\n    end\nend\n__tome_path_refresh\nset -gx TOME_BIN {bin_dir}\nset -gx TOME_SOCKET {socket}\n",
		bin_dir = bin_dir.display(),
		socket = socket_path.display(),
	));
	script.push_str("set -gx TOME_FISH_CONFIG 1\n");
	script.push_str(fish_command_not_found_script());
	append_fish_command_functions(&mut script);

	fs::write(fish_dir.join("config.fish"), script)?;
	Ok(Some(fish_dir))
}

fn find_original_fish_config() -> Option<PathBuf> {
	let config_dir = if let Some(dir) = std::env::var_os("FISH_CONFIG_DIR") {
		PathBuf::from(dir)
	} else if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
		PathBuf::from(dir).join("fish")
	} else if let Some(home) = std::env::var_os("HOME") {
		PathBuf::from(home).join(".config/fish")
	} else {
		return None;
	};

	let config = config_dir.join("config.fish");
	if config.exists() { Some(config) } else { None }
}

fn find_in_path(bin: &str, exclude_dir: Option<&Path>) -> Option<PathBuf> {
	let path = std::env::var_os("PATH")?;
	for dir in std::env::split_paths(&path) {
		if exclude_dir.is_some_and(|ex| dir == ex) {
			continue;
		}
		let candidate = dir.join(bin);
		if candidate.is_file() {
			return Some(candidate);
		}
	}
	None
}

pub(crate) fn fish_init_command(bin_dir: &Path, socket_path: &Path) -> String {
	let handler = fish_command_not_found_inline();
	format!(
		"function __tome_path_refresh --on-event fish_prompt; \
if not contains -- {bin_dir} $PATH; set -gx PATH {bin_dir} $PATH; end; \
end; __tome_path_refresh; set -gx TOME_BIN {bin_dir}; set -gx TOME_SOCKET {socket}; set -gx TOME_FISH_CONFIG 1; {handler}",
		bin_dir = bin_dir.display(),
		socket = socket_path.display(),
		handler = handler,
	)
}

/// Generates bash initialization commands for Tome IPC integration.
///
/// Sets up PATH, TOME_BIN, and TOME_SOCKET, plus a command_not_found_handle
/// function to intercept `:command` invocations.
pub fn bash_init_command(bin_dir: &Path, socket_path: &Path) -> String {
	format!(
		r#"export TOME_BIN="{bin_dir}"; export TOME_SOCKET="{socket}"; export PATH="$TOME_BIN:$PATH"; command_not_found_handle() {{ local cmd="$1"; shift; if [[ "$cmd" == :* ]] && [[ -x "$TOME_BIN/$cmd" ]]; then "$TOME_BIN/$cmd" "$@"; return $?; fi; echo "bash: $cmd: command not found" >&2; return 127; }}"#,
		bin_dir = bin_dir.display(),
		socket = socket_path.display(),
	)
}

/// Generates zsh initialization commands for Tome IPC integration.
///
/// Sets up PATH, TOME_BIN, and TOME_SOCKET, plus a command_not_found_handler
/// function to intercept `:command` invocations.
pub fn zsh_init_command(bin_dir: &Path, socket_path: &Path) -> String {
	format!(
		r#"export TOME_BIN="{bin_dir}"; export TOME_SOCKET="{socket}"; export PATH="$TOME_BIN:$PATH"; command_not_found_handler() {{ local cmd="$1"; shift; if [[ "$cmd" == :* ]] && [[ -x "$TOME_BIN/$cmd" ]]; then "$TOME_BIN/$cmd" "$@"; return $?; fi; echo "zsh: command not found: $cmd" >&2; return 127; }}"#,
		bin_dir = bin_dir.display(),
		socket = socket_path.display(),
	)
}

/// Generates nushell initialization commands for Tome IPC integration.
///
/// Sets up PATH, TOME_BIN, and TOME_SOCKET environment variables.
/// Note: nushell doesn't support command_not_found hooks in the same way,
/// so users must invoke commands via `^$env.TOME_BIN/:write` or similar.
pub fn nushell_init_command(bin_dir: &Path, socket_path: &Path) -> String {
	format!(
		r#"$env.TOME_BIN = "{bin_dir}"; $env.TOME_SOCKET = "{socket}"; $env.PATH = ($env.PATH | prepend "{bin_dir}")"#,
		bin_dir = bin_dir.display(),
		socket = socket_path.display(),
	)
}

fn fish_command_not_found_script() -> &'static str {
	"function fish_command_not_found\n    set -l cmd $argv[1]\n    if string match -q ':*' -- $cmd\n        set -l target \"$TOME_BIN/$cmd\"\n        if test -x \"$target\"\n            \"$target\" $argv[2..-1]\n            return $status\n        end\n    end\n    if functions -q __fish_command_not_found_handler\n        __fish_command_not_found_handler $argv\n    end\n    return 127\nend\n"
}

fn fish_command_not_found_inline() -> String {
	fish_command_not_found_script()
		.lines()
		.collect::<Vec<_>>()
		.join("; ")
}

fn append_fish_command_functions(script: &mut String) {
	for cmd in COMMANDS.iter() {
		append_fish_command_function(script, cmd.name);
		for alias in cmd.aliases {
			append_fish_command_function(script, alias);
		}
	}
}

fn append_fish_command_function(script: &mut String, name: &str) {
	script.push_str(&format!(
		"function :{name}\n    \"$TOME_BIN/:{name}\" $argv\nend\n",
	));
}

fn run_listener(listener: UnixListener, tx: Sender<IpcRequest>) {
	loop {
		match listener.accept() {
			Ok((stream, _)) => {
				if let Some(req) = parse_request(stream)
					&& tx.send(req).is_err()
				{
					break;
				}
			}
			Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
				thread::sleep(std::time::Duration::from_millis(10));
			}
			Err(_) => break,
		}
	}
}

fn parse_request(stream: UnixStream) -> Option<IpcRequest> {
	let reader = BufReader::new(stream);
	for line in reader.lines().map_while(Result::ok) {
		if line.is_empty() {
			continue;
		}
		let mut parts = line.split('\t');
		let command = parts.next()?.to_string();
		let args = parts.map(String::from).collect();
		return Some(IpcRequest { command, args });
	}
	None
}

#[cfg(test)]
mod tests {
	use std::io::Write;

	use super::*;

	#[test]
	fn wrapper_script_has_required_elements() {
		let script = generate_wrapper_script("write");
		assert!(script.starts_with("#!/bin/sh"));
		assert!(script.contains("TOME_SOCKET"));
		assert!(script.contains("write"));
	}

	#[test]
	fn ipc_creates_socket_and_bin_dir() {
		let ipc = TerminalIpc::new().unwrap();
		assert!(ipc.env.bin_dir.is_dir());
		assert!(ipc.env.socket_path.exists());

		let env_vars = ipc.env.env_vars();
		assert!(
			env_vars
				.iter()
				.any(|(k, v)| k == "PATH" && v.contains("tome-"))
		);
		assert!(env_vars.iter().any(|(k, _)| k == "TOME_SOCKET"));
	}

	#[test]
	fn env_is_sync_send() {
		fn assert_sync_send<T: Sync + Send>() {}
		assert_sync_send::<TerminalIpcEnv>();
	}

	#[test]
	fn socket_receives_command_with_arg() {
		let mut ipc = TerminalIpc::new().unwrap();

		let mut stream = UnixStream::connect(ipc.env.socket_path()).unwrap();
		writeln!(stream, "write\t/tmp/test.txt").unwrap();
		drop(stream);
		thread::sleep(std::time::Duration::from_millis(50));

		let req = ipc.poll().expect("should receive request");
		assert_eq!(req.command, "write");
		assert_eq!(req.args, vec!["/tmp/test.txt"]);
	}

	#[test]
	fn socket_receives_command_without_args() {
		let mut ipc = TerminalIpc::new().unwrap();

		let mut stream = UnixStream::connect(ipc.env.socket_path()).unwrap();
		writeln!(stream, "quit").unwrap();
		drop(stream);
		thread::sleep(std::time::Duration::from_millis(50));

		let req = ipc.poll().expect("should receive request");
		assert_eq!(req.command, "quit");
		assert!(req.args.is_empty());
	}

	#[test]
	fn socket_receives_command_with_multiple_args() {
		let mut ipc = TerminalIpc::new().unwrap();

		let mut stream = UnixStream::connect(ipc.env.socket_path()).unwrap();
		writeln!(stream, "edit\tfile1.rs\tfile2.rs\tfile3.rs").unwrap();
		drop(stream);
		thread::sleep(std::time::Duration::from_millis(50));

		let req = ipc.poll().expect("should receive request");
		assert_eq!(req.command, "edit");
		assert_eq!(req.args, vec!["file1.rs", "file2.rs", "file3.rs"]);
	}

	#[test]
	fn command_wrappers_exist_for_registered_commands() {
		let ipc = TerminalIpc::new().unwrap();

		// Print all registered commands for debugging
		let all_commands: Vec<_> = COMMANDS.iter().map(|c| c.name).collect();
		eprintln!("Registered commands: {:?}", all_commands);
		eprintln!("bin_dir: {:?}", ipc.env.bin_dir());

		for name in ["write", "quit", "help"] {
			if tome_manifest::find_command(name).is_some() {
				let wrapper = ipc.env.bin_dir().join(format!(":{}", name));
				assert!(wrapper.exists(), "wrapper for :{} should exist", name);
			}
		}
	}

	#[test]
	fn env_vars_contain_bin_dir_in_path() {
		let ipc = TerminalIpc::new().unwrap();
		let env_vars = ipc.env.env_vars();

		let path_var = env_vars.iter().find(|(k, _)| k == "PATH").unwrap();
		assert!(
			path_var
				.1
				.contains(&ipc.env.bin_dir().display().to_string()),
			"PATH should contain bin_dir: {}",
			path_var.1
		);
	}

	#[test]
	fn wrapper_script_is_executable_and_valid() {
		let ipc = TerminalIpc::new().unwrap();
		let wrapper = ipc.env.bin_dir().join(":write");

		if wrapper.exists() {
			let content = std::fs::read_to_string(&wrapper).unwrap();
			assert!(content.starts_with("#!/bin/sh"), "should have shebang");
			assert!(content.contains("write"), "should contain command name");

			let metadata = std::fs::metadata(&wrapper).unwrap();
			use std::os::unix::fs::PermissionsExt;
			assert!(
				metadata.permissions().mode() & 0o111 != 0,
				"should be executable"
			);
		}
	}

	#[test]
	fn spawned_process_can_find_wrapper_in_path() {
		let ipc = TerminalIpc::new().unwrap();
		let env_vars = ipc.env.env_vars();

		// Spawn a shell that checks if :write is in PATH
		let output = std::process::Command::new("sh")
			.arg("-c")
			.arg("command -v :write")
			.envs(env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())))
			.output()
			.unwrap();

		let stdout = String::from_utf8_lossy(&output.stdout);
		assert!(
			output.status.success(),
			":write should be found in PATH. stdout={}, stderr={}",
			stdout,
			String::from_utf8_lossy(&output.stderr)
		);
		assert!(
			stdout.contains(":write"),
			"output should contain :write path"
		);
	}
}
