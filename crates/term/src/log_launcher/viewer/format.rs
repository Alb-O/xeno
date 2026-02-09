pub fn dim(s: &str) -> String {
	format!("\x1b[90m{}\x1b[0m", s)
}

pub fn cyan(s: &str) -> String {
	format!("\x1b[36m{}\x1b[0m", s)
}

pub fn format_duration(us: u64) -> String {
	if us > 1_000_000 {
		format!("{:.2}s", us as f64 / 1_000_000.0)
	} else if us > 1_000 {
		format!("{:.2}ms", us as f64 / 1_000.0)
	} else {
		format!("{}us", us)
	}
}

pub fn format_relative_time(ms: u64) -> String {
	if ms >= 60_000 {
		format!("{:>2}:{:02}", ms / 60_000, (ms % 60_000) / 1000)
	} else {
		format!("{:>5.1}s", ms as f64 / 1000.0)
	}
}

/// Strips `xeno_*` crate prefix from target paths.
///
/// Examples: `xeno_lsp::registry` → `registry`, `xeno_api::editor::ops` → `editor::ops`
pub fn truncate_target(target: &str) -> String {
	if let Some(rest) = target.strip_prefix("xeno_")
		&& let Some(pos) = rest.find("::")
	{
		return rest[pos + 2..].to_string();
	}
	target.to_string()
}
