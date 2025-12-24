use std::io::{self, Write};
use std::time::Duration;

use termina::escape::csi::{
	Csi, Cursor, DecPrivateMode, DecPrivateModeCode, Keyboard, KittyKeyboardFlags, Mode,
};
use termina::event::Event;
use termina::style::CursorStyle;
use termina::{EventReader, PlatformTerminal, Terminal as _, WindowSize};

pub fn enable_terminal_features(terminal: &mut PlatformTerminal) -> io::Result<()> {
	terminal.enter_raw_mode()?;
	write!(
		terminal,
		"{}{}",
		Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
			DecPrivateModeCode::ClearAndEnableAlternateScreen
		))),
		Csi::Keyboard(Keyboard::PushFlags(
			KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES
		))
	)?;
	write!(
		terminal,
		"{}{}{}",
		Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
			DecPrivateModeCode::MouseTracking
		))),
		Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
			DecPrivateModeCode::SGRMouse
		))),
		Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
			DecPrivateModeCode::AnyEventMouse
		))),
	)?;
	terminal.flush()
}

pub fn disable_terminal_features(terminal: &mut PlatformTerminal) -> io::Result<()> {
	write!(
		terminal,
		"{}{}{}{}{}{}",
		Csi::Cursor(Cursor::CursorStyle(CursorStyle::Default)),
		Csi::Keyboard(Keyboard::PopFlags(1)),
		Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
			DecPrivateModeCode::MouseTracking
		))),
		Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
			DecPrivateModeCode::SGRMouse
		))),
		Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
			DecPrivateModeCode::AnyEventMouse
		))),
		Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
			DecPrivateModeCode::ClearAndEnableAlternateScreen
		)))
	)?;
	terminal.enter_cooked_mode()?;
	terminal.flush()
}

pub fn install_panic_hook(terminal: &mut PlatformTerminal) {
	terminal.set_panic_hook(|handle| {
		let _ = write!(
			handle,
			"{}{}{}{}{}{}",
			Csi::Cursor(Cursor::CursorStyle(CursorStyle::Default)),
			Csi::Keyboard(Keyboard::PopFlags(1)),
			Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
				DecPrivateModeCode::MouseTracking
			))),
			Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
				DecPrivateModeCode::SGRMouse
			))),
			Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
				DecPrivateModeCode::AnyEventMouse
			))),
			Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(
				DecPrivateModeCode::ClearAndEnableAlternateScreen
			)))
		);
		let _ = handle.flush();
	});
}

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

pub fn cursor_style_for_mode(mode: tome_core::Mode) -> CursorStyle {
	match mode {
		tome_core::Mode::Insert => CursorStyle::BlinkingBar,
		_ => CursorStyle::SteadyBlock,
	}
}
