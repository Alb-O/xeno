use evildoer_manifest::editor_ctx::MessageAccess;

pub trait NotifyINFOExt: MessageAccess {
	fn info(&mut self, msg: &str) {
		self.notify("info", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifyINFOExt for T {}

pub trait NotifyWARNExt: MessageAccess {
	fn warn(&mut self, msg: &str) {
		self.notify("warn", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifyWARNExt for T {}

pub trait NotifyERRORExt: MessageAccess {
	fn error(&mut self, msg: &str) {
		self.notify("error", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifyERRORExt for T {}

pub trait NotifySUCCESSExt: MessageAccess {
	fn success(&mut self, msg: &str) {
		self.notify("success", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifySUCCESSExt for T {}

pub trait NotifyDEBUGExt: MessageAccess {
	fn debug(&mut self, msg: &str) {
		self.notify("debug", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifyDEBUGExt for T {}
