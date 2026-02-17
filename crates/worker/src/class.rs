/// Shared execution classes used for worker scheduling and observability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskClass {
	/// Latency-sensitive work that directly affects interactive UX.
	Interactive,
	/// Background async work that can be delayed or dropped under pressure.
	Background,
	/// Blocking I/O work executed on blocking pools or dedicated threads.
	IoBlocking,
	/// CPU-intensive blocking work executed on blocking pools or dedicated threads.
	CpuBlocking,
}

impl TaskClass {
	pub(crate) const fn as_str(self) -> &'static str {
		match self {
			Self::Interactive => "interactive",
			Self::Background => "background",
			Self::IoBlocking => "io_blocking",
			Self::CpuBlocking => "cpu_blocking",
		}
	}
}
