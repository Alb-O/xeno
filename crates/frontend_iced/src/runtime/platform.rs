#[cfg(target_os = "linux")]
pub(crate) fn configure_linux_backend() {
	if std::env::var_os("WINIT_UNIX_BACKEND").is_some() {
		return;
	}

	if let Some(requested) = std::env::var("XENO_ICED_BACKEND").ok().map(|value| value.to_lowercase())
		&& matches!(requested.as_str(), "x11" | "wayland")
	{
		set_winit_unix_backend(&requested);
		return;
	}

	if std::env::var_os("WAYLAND_DISPLAY").is_some() {
		set_winit_unix_backend("wayland");
		return;
	}

	if std::env::var_os("DISPLAY").is_some() {
		set_winit_unix_backend("x11");
	}
}

#[cfg(target_os = "linux")]
fn set_winit_unix_backend(value: &str) {
	unsafe {
		// SAFETY: This runs before iced/winit event-loop initialization and before
		// runtime task spawning, so no concurrent environment access occurs here.
		std::env::set_var("WINIT_UNIX_BACKEND", value);
	}
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn configure_linux_backend() {}
