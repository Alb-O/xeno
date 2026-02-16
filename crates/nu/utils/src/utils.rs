use std::io;

#[cfg(windows)]
use crossterm_winapi::{ConsoleMode, Handle};

pub fn enable_vt_processing() -> io::Result<()> {
	#[cfg(windows)]
	{
		let console_out_mode = ConsoleMode::from(Handle::current_out_handle()?);
		let old_out_mode = console_out_mode.mode()?;
		let console_in_mode = ConsoleMode::from(Handle::current_in_handle()?);
		let old_in_mode = console_in_mode.mode()?;

		enable_vt_processing_input(console_in_mode, old_in_mode)?;
		enable_vt_processing_output(console_out_mode, old_out_mode)?;
	}
	Ok(())
}

#[cfg(windows)]
fn enable_vt_processing_input(console_in_mode: ConsoleMode, mode: u32) -> io::Result<()> {
	const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
	const ENABLE_LINE_INPUT: u32 = 0x0002;
	const ENABLE_ECHO_INPUT: u32 = 0x0004;
	const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;

	console_in_mode.set_mode(mode | ENABLE_VIRTUAL_TERMINAL_INPUT & ENABLE_ECHO_INPUT & ENABLE_LINE_INPUT & ENABLE_PROCESSED_INPUT)
}

#[cfg(windows)]
fn enable_vt_processing_output(console_out_mode: ConsoleMode, mode: u32) -> io::Result<()> {
	const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
	const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

	console_out_mode.set_mode(mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING)
}

/// Returns the terminal size (columns, rows).
///
/// This utility variant allows getting a fallback value when compiling for
/// wasm32 without rearranging call sites.
///
/// See [`crossterm::terminal::size`].
pub fn terminal_size() -> io::Result<(u16, u16)> {
	Err(io::Error::from(io::ErrorKind::Unsupported))
}
