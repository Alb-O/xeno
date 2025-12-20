use std::path::PathBuf;
use std::sync::mpsc;
use std::{fs, io};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::plugin::manager::get_config_dir;

pub enum IpcMessage {
	ReloadPlugin(String),
}

pub struct IpcServer {
	receiver: mpsc::Receiver<IpcMessage>,
}

impl IpcServer {
	pub fn start() -> io::Result<Self> {
		let (sender, receiver) = mpsc::channel();
		let socket_path =
			get_socket_path().ok_or_else(|| io::Error::other("Could not determine socket path"))?;

		// Remove existing socket if any
		if socket_path.exists() {
			let _ = fs::remove_file(&socket_path);
		}

		std::thread::spawn(move || {
			let rt = tokio::runtime::Builder::new_current_thread()
				.enable_all()
				.build()
				.unwrap();

			rt.block_on(async {
				let listener = UnixListener::bind(&socket_path).unwrap();
				loop {
					if let Ok((mut stream, _)) = listener.accept().await {
						let sender = sender.clone();
						tokio::spawn(async move {
							let mut buf = [0u8; 1024];
							if let Ok(n) = stream.read(&mut buf).await {
								let msg = String::from_utf8_lossy(&buf[..n]);
								handle_client_msg(&msg, sender);
							}
						});
					}
				}
			});
		});

		Ok(Self { receiver })
	}

	pub fn poll(&self) -> Option<IpcMessage> {
		self.receiver.try_recv().ok()
	}
}

fn handle_client_msg(msg: &str, sender: mpsc::Sender<IpcMessage>) {
	let parts: Vec<&str> = msg.split_whitespace().collect();
	if parts.is_empty() {
		return;
	}

	match parts[0] {
		"reload" if parts.len() > 1 => {
			let _ = sender.send(IpcMessage::ReloadPlugin(parts[1].to_string()));
		}
		_ => {}
	}
}

pub fn get_socket_path() -> Option<PathBuf> {
	get_config_dir().map(|d| d.join("tome.sock"))
}

pub async fn send_client_msg(msg: &str) -> io::Result<()> {
	let socket_path =
		get_socket_path().ok_or_else(|| io::Error::other("Could not determine socket path"))?;

	let mut stream = UnixStream::connect(socket_path).await?;
	stream.write_all(msg.as_bytes()).await?;
	Ok(())
}
