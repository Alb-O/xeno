use super::HookContext;
use crate::hook;

hook!(
	log_buffer_open,
	BufferOpen,
	1000,
	"Log when a buffer is opened",
	|ctx| {
		if let HookContext::BufferOpen {
			path, file_type, ..
		} = ctx
		{
			let ft = file_type.unwrap_or("unknown");
			// In a real implementation, this would use a proper logging system
			let _ = (path, ft);
		}
	}
);
