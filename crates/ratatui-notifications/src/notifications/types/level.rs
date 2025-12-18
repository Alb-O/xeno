/// Severity level of a notification.
///
/// Affects the visual styling of the notification (colors, borders).
/// Higher severity levels typically use more prominent colors to draw attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Level {
	/// Informational message (default).
	#[default]
	Info,

	/// Warning message.
	Warn,

	/// Error message.
	Error,

	/// Debug message.
	Debug,

	/// Trace message.
	Trace,
}
