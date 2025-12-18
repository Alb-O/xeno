use clap::builder::Styles;
use clap::builder::styling::AnsiColor;

/// Returns clap Styles configured to match cargo's help output colors.
pub fn cli_styles() -> Styles {
	Styles::styled()
		.header(AnsiColor::Green.on_default().bold())
		.usage(AnsiColor::Green.on_default().bold())
		.literal(AnsiColor::Cyan.on_default())
		.placeholder(AnsiColor::Cyan.on_default())
		.valid(AnsiColor::Cyan.on_default())
}
