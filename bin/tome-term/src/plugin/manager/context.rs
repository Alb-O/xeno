use std::cell::RefCell;

use super::PluginManager;
use crate::editor::Editor;

thread_local! {
	pub static ACTIVE_MANAGER: RefCell<Option<*mut PluginManager>> = const { RefCell::new(None) };
	pub static ACTIVE_EDITOR: RefCell<Option<*mut Editor>> = const { RefCell::new(None) };
}

pub struct PluginContextGuard {
	old_mgr: Option<*mut PluginManager>,
	old_ed: Option<*mut Editor>,
	old_plugin_id: Option<String>,
	mgr_ptr: *mut PluginManager,
}

impl PluginContextGuard {
	pub unsafe fn new(mgr_ptr: *mut PluginManager, ed_ptr: *mut Editor, plugin_id: &str) -> Self {
		let old_mgr = ACTIVE_MANAGER
			.with(|ctx: &RefCell<Option<*mut PluginManager>>| ctx.replace(Some(mgr_ptr)));
		let old_ed =
			ACTIVE_EDITOR.with(|ctx: &RefCell<Option<*mut Editor>>| ctx.replace(Some(ed_ptr)));
		let (old_plugin_id, mgr_ptr_ref) =
			unsafe { ((*mgr_ptr).current_plugin_id.clone(), &mut *mgr_ptr) };
		mgr_ptr_ref.current_plugin_id = Some(plugin_id.to_string());
		Self {
			old_mgr,
			old_ed,
			old_plugin_id,
			mgr_ptr,
		}
	}
}

impl Drop for PluginContextGuard {
	fn drop(&mut self) {
		ACTIVE_MANAGER.with(|ctx: &RefCell<Option<*mut PluginManager>>| ctx.replace(self.old_mgr));
		ACTIVE_EDITOR.with(|ctx: &RefCell<Option<*mut Editor>>| ctx.replace(self.old_ed));
		unsafe {
			(*self.mgr_ptr).current_plugin_id = self.old_plugin_id.take();
		}
	}
}
