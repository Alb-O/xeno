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
}
