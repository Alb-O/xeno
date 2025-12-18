use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use tui_term::vt100::Parser;

pub struct TerminalState {
	pub parser: Parser,
	pub pty_master: Box<dyn MasterPty + Send>,
	pub pty_writer: Box<dyn Write + Send>,
	pub receiver: Receiver<Vec<u8>>,
	// We keep child to ensure it stays alive and to check status if needed
	pub child: Box<dyn portable_pty::Child + Send>,
}

impl TerminalState {
	pub fn new(cols: u16, rows: u16) -> Result<Self, String> {
		let pty_system = NativePtySystem::default();
		let pair = pty_system
			.openpty(PtySize {
				rows,
				cols,
				pixel_width: 0,
				pixel_height: 0,
			})
			.map_err(|e| e.to_string())?;

		// Use shell from env or default to sh/bash
		let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
		let cmd = CommandBuilder::new(shell);

		let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;

		let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
		let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
		let master = pair.master;

		let (tx, rx) = channel();

		thread::spawn(move || {
			let mut buf = [0u8; 4096];
			loop {
				match reader.read(&mut buf) {
					Ok(n) if n > 0 => {
						if tx.send(buf[..n].to_vec()).is_err() {
							break;
						}
					}
					_ => break,
				}
			}
		});

		Ok(Self {
			parser: Parser::new(rows, cols, 0),
			pty_master: master,
			pty_writer: writer,
			receiver: rx,
			child,
		})
	}

	pub fn update(&mut self) {
		loop {
			match self.receiver.try_recv() {
				Ok(bytes) => {
					// Hack: Check for DA1 query (Primary Device Attributes) from shell (e.g. fish)
					// The query is \e[c or \e[0c.
					// If detected, we send a response manually because vt100 parser doesn't handle responses.
					// We look for the sequence in the incoming bytes.
					// This is simple substring search, might miss fragmented sequences but covers most startup cases.
					let da1_query = b"\x1b[c";
					let da1_query_0 = b"\x1b[0c";

					if bytes.windows(da1_query.len()).any(|w| w == da1_query)
						|| bytes.windows(da1_query_0.len()).any(|w| w == da1_query_0)
					{
						// Respond as VT102 (\e[?6c)
						let _ = self.pty_writer.write_all(b"\x1b[?6c");
					}

					self.parser.process(&bytes);
				}
				Err(TryRecvError::Empty) => break,
				Err(TryRecvError::Disconnected) => break,
			}
		}
	}

	pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), String> {
		self.parser.set_size(rows, cols);
		self.pty_master
			.resize(PtySize {
				rows,
				cols,
				pixel_width: 0,
				pixel_height: 0,
			})
			.map_err(|e| e.to_string())
	}

	pub fn write_key(&mut self, bytes: &[u8]) -> Result<(), String> {
		self.pty_writer.write_all(bytes).map_err(|e| e.to_string())
	}

	pub fn is_alive(&mut self) -> bool {
		match self.child.try_wait() {
			Ok(Some(_)) => false, // Exited
			Ok(None) => true,     // Still running
			Err(_) => false,      // Error presumably means dead or inaccessible
		}
	}
}
