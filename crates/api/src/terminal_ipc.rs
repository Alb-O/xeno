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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::Arc;
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

	/// Returns environment variables to set for spawned terminals.
	pub fn env_vars(&self) -> Vec<(String, String)> {
		let current_path = std::env::var("PATH").unwrap_or_default();
		vec![
			("PATH".to_string(), format!("{}:{}", self.bin_dir.display(), current_path)),
			("TOME_SOCKET".to_string(), self.socket_path.display().to_string()),
		]
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

		let _ = fs::remove_file(&socket_path);
		let listener = UnixListener::bind(&socket_path)?;
		listener.set_nonblocking(true)?;

		let (tx, rx) = channel();
		thread::spawn(move || run_listener(listener, tx));

		Ok(Self {
			env: Arc::new(TerminalIpcEnv { bin_dir, socket_path }),
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
echo "$MSG" | nc -U -q0 "$TOME_SOCKET" 2>/dev/null || \
echo "$MSG" | nc -U "$TOME_SOCKET" 2>/dev/null || \
printf '%s\n' "$MSG" | socat - UNIX-CONNECT:"$TOME_SOCKET" 2>/dev/null
"#
	)
}

fn run_listener(listener: UnixListener, tx: Sender<IpcRequest>) {
	loop {
		match listener.accept() {
			Ok((stream, _)) => {
				if let Some(req) = parse_request(stream) {
					if tx.send(req).is_err() {
						break;
					}
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
		assert!(env_vars.iter().any(|(k, v)| k == "PATH" && v.contains("tome-")));
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

		for name in ["write", "quit", "help"] {
			if tome_manifest::find_command(name).is_some() {
				let wrapper = ipc.env.bin_dir().join(format!(":{}", name));
				assert!(wrapper.exists(), "wrapper for :{} should exist", name);
			}
		}
	}
}
