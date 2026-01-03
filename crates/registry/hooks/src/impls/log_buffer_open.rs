use crate::{HookEventData, hook};

hook!(
	log_buffer_open,
	BufferOpen,
	1000,
	"Log when a buffer is opened",
	|ctx| {
		if let HookEventData::BufferOpen {
			path, file_type, ..
		} = &ctx.data
		{
			let ft = file_type.unwrap_or("unknown");
			// In a real implementation, this would use a proper logging system
			let _ = (path, ft);
		}
	}
);
