//! Terminal setup and control utilities.
//!
//! Provides functions for enabling/disabling terminal features like raw mode,
//! alternate screen, mouse tracking, and keyboard enhancement protocols.

use std::io::{self, Write};
use std::time::Duration;

use termina::escape::csi::{
	Csi, Cursor, DecPrivateMode, DecPrivateModeCode, Keyboard, KittyKeyboardFlags, Mode,
};
use termina::event::Event;
use termina::style::CursorStyle;
use termina::{EventReader, PlatformTerminal, Terminal as _, WindowSize};
use xeno_core::{TerminalConfig, TerminalSequence};

/// Writes terminal escape sequences to a writer.
fn write_sequences<W: io::Write>(writer: &mut W, sequences: &[TerminalSequence]) -> io::Result<()> {
	for sequence in sequences {
		write!(writer, "{}", sequence_to_csi(*sequence))?;
	}
	Ok(())
}

/// Converts a terminal sequence enum to a CSI escape sequence.
fn sequence_to_csi(sequence: TerminalSequence) -> Csi {
	match sequence {
		TerminalSequence::EnableAlternateScreen => Csi::Mode(Mode::SetDecPrivateMode(
			DecPrivateMode::Code(DecPrivateModeCode::ClearAndEnableAlternateScreen),
		)),
		TerminalSequence::DisableAlternateScreen => Csi::Mode(Mode::ResetDecPrivateMode(
			DecPrivateMode::Code(DecPrivateModeCode::ClearAndEnableAlternateScreen),
		)),
		TerminalSequence::EnableMouseTracking => Csi::Mode(Mode::SetDecPrivateMode(
			DecPrivateMode::Code(DecPrivateModeCode::MouseTracking),
		)),
		TerminalSequence::DisableMouseTracking => Csi::Mode(Mode::ResetDecPrivateMode(
			DecPrivateMode::Code(DecPrivateModeCode::MouseTracking),
		)),
		TerminalSequence::EnableSgrMouse => Csi::Mode(Mode::SetDecPrivateMode(
			DecPrivateMode::Code(DecPrivateModeCode::SGRMouse),
		)),
		TerminalSequence::DisableSgrMouse => Csi::Mode(Mode::ResetDecPrivateMode(
			DecPrivateMode::Code(DecPrivateModeCode::SGRMouse),
		)),
		TerminalSequence::EnableAnyEventMouse => Csi::Mode(Mode::SetDecPrivateMode(
			DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse),
		)),
		TerminalSequence::DisableAnyEventMouse => Csi::Mode(Mode::ResetDecPrivateMode(
			DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse),
		)),
		TerminalSequence::PushKittyKeyboardDisambiguate => Csi::Keyboard(Keyboard::PushFlags(
			KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES,
		)),
		TerminalSequence::PopKittyKeyboardFlags => Csi::Keyboard(Keyboard::PopFlags(1)),
		TerminalSequence::ResetCursorStyle => {
			Csi::Cursor(Cursor::CursorStyle(CursorStyle::Default))
		}
	}
}

/// Enables terminal features using auto-detected configuration.
pub fn enable_terminal_features(terminal: &mut PlatformTerminal) -> io::Result<()> {
	enable_terminal_features_with_config(terminal, TerminalConfig::detect())
}

/// Enables terminal features using the provided configuration.
pub fn enable_terminal_features_with_config(
	terminal: &mut PlatformTerminal,
	config: TerminalConfig,
) -> io::Result<()> {
	terminal.enter_raw_mode()?;
	write_sequences(terminal, config.enter_sequences)?;
	terminal.flush()
}

/// Disables terminal features using auto-detected configuration.
pub fn disable_terminal_features(terminal: &mut PlatformTerminal) -> io::Result<()> {
	disable_terminal_features_with_config(terminal, TerminalConfig::detect())
}

/// Disables terminal features using the provided configuration.
pub fn disable_terminal_features_with_config(
	terminal: &mut PlatformTerminal,
	config: TerminalConfig,
) -> io::Result<()> {
	write_sequences(terminal, config.exit_sequences)?;
	terminal.enter_cooked_mode()?;
	terminal.flush()
}

/// Installs a panic hook to restore terminal state on panic.
pub fn install_panic_hook(terminal: &mut PlatformTerminal) {
	install_panic_hook_with_config(terminal, TerminalConfig::detect());
}

/// Installs a panic hook with the provided configuration.
pub fn install_panic_hook_with_config(terminal: &mut PlatformTerminal, config: TerminalConfig) {
	terminal.set_panic_hook(move |handle| {
		let _ = write_sequences(handle, config.panic_sequences);
		let _ = handle.flush();
	});
}

/// Coalesces multiple resize events into a single final size.
pub fn coalesce_resize_events(events: &EventReader, first: WindowSize) -> io::Result<WindowSize> {
	let mut filter = |event: &Event| matches!(event, Event::WindowResized(_));
	let mut latest = first;

	while events.poll(Some(Duration::from_millis(0)), &mut filter)? {
		if let Event::WindowResized(size) = events.read(&mut filter)? {
			latest = size;
		}
	}

	Ok(latest)
}

/// Returns the appropriate cursor style for the given editor mode.
pub fn cursor_style_for_mode(mode: xeno_base::Mode) -> CursorStyle {
	match mode {
		xeno_base::Mode::Insert => CursorStyle::SteadyBar,
		_ => CursorStyle::SteadyBlock,
	}
}
