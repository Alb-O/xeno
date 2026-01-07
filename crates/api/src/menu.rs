//! Application menu bar.

use xeno_tui::widgets::menu::{MenuEvent, MenuItem, MenuState};

/// Action triggered by menu item selection.
#[derive(Debug, Clone)]
pub enum MenuAction {
	/// Execute a command by name.
	Command(&'static str),
}

/// Creates the default application menu bar.
pub fn create_menu() -> MenuState<MenuAction> {
	use xeno_registry::menus::{MENU_GROUPS, MENU_ITEMS};

	let mut groups: Vec<_> = MENU_GROUPS.iter().collect();
	groups.sort_by_key(|group| group.priority);

	let menu_items: Vec<MenuItem<MenuAction>> = groups
		.into_iter()
		.map(|group| {
			let mut items: Vec<_> = MENU_ITEMS
				.iter()
				.filter(|item| item.group == group.name)
				.collect();
			items.sort_by_key(|item| item.priority);

			let children = items
				.into_iter()
				.map(|item| {
					let menu_item = MenuItem::item(item.label, MenuAction::Command(item.command));
					match item.icon {
						Some(codepoint) => menu_item.icon_codepoint(codepoint),
						None => menu_item,
					}
				})
				.collect();

			MenuItem::group(group.label, children)
		})
		.collect();

	MenuState::new(menu_items)
}

/// Processes menu events and queues corresponding commands.
pub fn process_menu_events(
	menu: &mut MenuState<MenuAction>,
	command_queue: &mut crate::editor::CommandQueue,
) {
	let mut had_selection = false;
	for event in menu.drain_events() {
		had_selection = true;
		match event {
			MenuEvent::Selected(MenuAction::Command(cmd)) => {
				command_queue.push(cmd, vec![]);
			}
		}
	}
	if had_selection {
		menu.reset();
	}
}
