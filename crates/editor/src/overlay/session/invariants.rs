use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use xeno_primitives::{Key, KeyCode, Modifiers, Selection};

use crate::geometry::Rect;
use crate::overlay::spec::RectPolicy;
use crate::overlay::{CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayStatus, OverlayUiSpec, WindowRole, WindowSpec};
use crate::window::GutterSelector;

struct ReflowTestOverlay;

impl OverlayController for ReflowTestOverlay {
	fn name(&self) -> &'static str {
		"ReflowTest"
	}

	fn ui_spec(&self, _ctx: &dyn OverlayContext) -> OverlayUiSpec {
		OverlayUiSpec {
			title: Some("ReflowTest".to_string()),
			gutter: GutterSelector::Prompt('>'),
			rect: RectPolicy::TopCenter {
				width_percent: 100,
				max_width: u16::MAX,
				min_width: 1,
				y_frac: (0, 1),
				height: 1,
			},
			style: crate::overlay::docked_prompt_style(),
			windows: vec![WindowSpec {
				role: WindowRole::List,
				rect: RectPolicy::Below(WindowRole::Input, 1, 1),
				style: crate::overlay::docked_prompt_style(),
				buffer_options: HashMap::new(),
				dismiss_on_blur: true,
				sticky: false,
				gutter: GutterSelector::Hidden,
			}],
		}
	}

	fn on_open(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession) {}

	fn on_input_changed(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _text: &str) {}

	fn on_commit<'a>(&'a mut self, _ctx: &'a mut dyn OverlayContext, _session: &'a mut OverlaySession) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		Box::pin(async {})
	}

	fn on_close(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _reason: CloseReason) {}
}

/// Must route non-overlay module access through `OverlaySystem` accessors.
///
/// * Enforced in: `OverlaySystem::{interaction,interaction_mut,take_interaction,restore_interaction,layers,layers_mut,store,store_mut}`
/// * Failure symptom: Callers couple to `OverlaySystem` field layout and crate-split refactors become non-local.
#[cfg_attr(test, test)]
pub(crate) fn test_overlay_system_accessors_round_trip() {
	let mut system = crate::overlay::OverlaySystem::new();

	assert!(!system.interaction().is_open());
	assert!(!system.interaction_mut().is_open());
	assert!(system.store().get::<String>().is_none());
	let _ = system.layers();
	let _ = system.layers_mut();

	let interaction = system.take_interaction();
	assert!(!interaction.is_open());
	system.restore_interaction(interaction);

	assert!(!system.interaction().is_open());
}

/// Must gate state restoration on captured buffer version matching.
///
/// * Enforced in: `OverlaySession::restore_all`
/// * Failure symptom: Stale cursor/selection state is restored over user's edits.
#[cfg_attr(test, test)]
pub(crate) fn test_versioned_restore() {
	let mut editor = crate::Editor::new_scratch();
	let view = editor.focused_view();

	{
		let buffer = editor.state.core.buffers.get_buffer_mut(view).expect("focused buffer must exist");
		buffer.set_cursor_and_selection(0, Selection::single(0, 0));
	}

	let mut session = OverlaySession {
		panes: Vec::new(),
		buffers: Vec::new(),
		input: view,
		origin_focus: editor.focus().clone(),
		origin_mode: editor.mode(),
		origin_view: view,
		capture: Default::default(),
		status: OverlayStatus::default(),
	};
	session.capture_view(&editor, view);

	{
		let buffer = editor.state.core.buffers.get_buffer_mut(view).expect("focused buffer must exist");
		buffer.reset_content("changed");
		buffer.set_cursor_and_selection(3, Selection::single(3, 3));
	}

	session.restore_all(&mut editor);

	let buffer = editor.state.core.buffers.get_buffer(view).expect("focused buffer must exist");
	assert_eq!(buffer.cursor, 3);
	assert_eq!(buffer.selection, Selection::single(3, 3));
}

/// Must allow only one active modal session at a time.
///
/// * Enforced in: `OverlayManager::open`
/// * Failure symptom: Two modal overlays fight for focus and input.
#[cfg_attr(test, test)]
pub(crate) fn test_exclusive_modal() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(100, 40);

	assert!(editor.open_command_palette());
	assert!(editor.state.overlay_system.interaction().is_open());
	assert!(!editor.open_command_palette());
}

/// Must clamp resolved overlay areas to screen bounds.
///
/// * Enforced in: `RectPolicy::resolve_opt`
/// * Failure symptom: Overlay panes extend beyond screen bounds.
#[cfg_attr(test, test)]
pub(crate) fn test_rect_policy_clamps_to_screen() {
	let screen = Rect::new(0, 0, 100, 50);
	let roles = HashMap::new();

	let policy = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 80,
		min_width: 20,
		y_frac: (1, 4),
		height: 10,
	};
	let rect = policy.resolve_opt(screen, &roles).unwrap();
	assert!(rect.x + rect.width <= screen.x + screen.width);
	assert!(rect.y + rect.height <= screen.y + screen.height);

	let policy_low = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 80,
		min_width: 20,
		y_frac: (9, 10),
		height: 20,
	};
	let rect_low = policy_low.resolve_opt(screen, &roles).unwrap();
	assert!(rect_low.y + rect_low.height <= screen.y + screen.height, "rect must be shifted up to fit");

	let zero = Rect::new(0, 0, 0, 0);
	assert!(policy.resolve_opt(zero, &roles).is_none());
}

/// Must finalize all session buffers during teardown.
///
/// * Enforced in: `OverlaySession::teardown`
/// * Failure symptom: Scratch buffers leak after overlays close.
#[cfg_attr(test, test)]
pub(crate) fn test_session_teardown_finalizes_buffers() {
	let mut editor = crate::Editor::new_scratch();
	let b1 = editor.state.core.buffers.create_scratch();
	let b2 = editor.state.core.buffers.create_scratch();
	let view = editor.focused_view();

	let mut session = OverlaySession {
		panes: Vec::new(),
		buffers: vec![b1, b2],
		input: b1,
		origin_focus: editor.focus().clone(),
		origin_mode: editor.mode(),
		origin_view: view,
		capture: Default::default(),
		status: OverlayStatus::default(),
	};

	session.teardown(&mut editor);

	assert!(session.buffers.is_empty());
	assert!(editor.state.core.buffers.get_buffer(b1).is_none());
	assert!(editor.state.core.buffers.get_buffer(b2).is_none());
}

/// Must reflow modal overlay panes on viewport resize.
///
/// * Enforced in: `OverlayManager::on_viewport_changed`
/// * Failure symptom: Open modals render with stale geometry after terminal resize.
#[cfg_attr(test, test)]
pub(crate) fn test_modal_reflow_on_resize() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let before = editor
		.state
		.overlay_system
		.interaction()
		.active()
		.and_then(|active| active.session.panes.first())
		.map(|pane| pane.rect)
		.expect("overlay pane should exist");

	editor.handle_window_resize(60, 20);

	let after = editor
		.state
		.overlay_system
		.interaction()
		.active()
		.and_then(|active| active.session.panes.first())
		.map(|pane| pane.rect)
		.expect("overlay pane should still exist");

	assert_ne!(before, after, "pane rect should reflow after resize");
	assert!(after.x + after.width <= 60);
	assert!(after.y + after.height <= 20);
}

/// Must clear auxiliary panes that cannot resolve after viewport shrink.
///
/// * Enforced in: `OverlayHost::reflow_session`
/// * Failure symptom: Auxiliary overlays keep stale geometry after resize.
#[cfg_attr(test, test)]
pub(crate) fn test_modal_reflow_clears_unresolved_aux_panes() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(100, 40);

	let mut interaction = editor.state.overlay_system.take_interaction();
	assert!(interaction.open(&mut editor, Box::new(ReflowTestOverlay)));
	editor.state.overlay_system.restore_interaction(interaction);

	let active = editor.state.overlay_system.interaction().active().expect("overlay should be open");
	let input_rect = active
		.session
		.panes
		.iter()
		.find(|pane| pane.role == WindowRole::Input)
		.expect("input pane must exist")
		.rect;
	let list_rect = active
		.session
		.panes
		.iter()
		.find(|pane| pane.role == WindowRole::List)
		.expect("list pane must exist")
		.rect;
	assert!(input_rect.width > 0 && input_rect.height > 0);
	assert!(list_rect.width > 0 && list_rect.height > 0);

	editor.handle_window_resize(100, 2);

	let active = editor.state.overlay_system.interaction().active().expect("overlay should remain open");
	let input_rect = active
		.session
		.panes
		.iter()
		.find(|pane| pane.role == WindowRole::Input)
		.expect("input pane must exist")
		.rect;
	let list_rect = active
		.session
		.panes
		.iter()
		.find(|pane| pane.role == WindowRole::List)
		.expect("list pane must exist")
		.rect;

	assert!(input_rect.width > 0 && input_rect.height > 0);
	assert_eq!(list_rect, Rect::new(0, 0, 0, 0));
}

/// Must restore origin focus on forced overlay close.
///
/// * Enforced in: `OverlayHost::cleanup_session`
/// * Failure symptom: Focus remains on stale overlay target after forced close.
#[cfg_attr(test, test)]
pub(crate) fn test_forced_close_restores_origin_focus() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let origin_focus = editor
		.state
		.overlay_system
		.interaction()
		.active()
		.map(|active| active.session.origin_focus.clone())
		.expect("overlay should be open");

	let mut interaction = editor.state.overlay_system.take_interaction();
	interaction.close(&mut editor, CloseReason::Forced);
	editor.state.overlay_system.restore_interaction(interaction);

	assert_eq!(*editor.focus(), origin_focus);
}

/// Must keep window manager state fixed to the base window for modal UI paths.
///
/// * Enforced in: `OverlayHost::setup_session`
/// * Failure symptom: Overlay UI mutates window manager state unexpectedly.
#[cfg_attr(test, test)]
pub(crate) fn test_modal_ui_keeps_single_base_window() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(100, 40);

	assert_eq!(editor.state.windows.windows().count(), 1);
	assert!(editor.open_command_palette());
	assert_eq!(editor.state.windows.windows().count(), 1);

	editor.interaction_cancel();
	assert_eq!(editor.state.windows.windows().count(), 1);
}

fn key_down() -> Key {
	Key {
		code: KeyCode::Down,
		modifiers: Modifiers::NONE,
	}
}

fn key_tab() -> Key {
	Key {
		code: KeyCode::Tab,
		modifiers: Modifiers::NONE,
	}
}

fn key_enter() -> Key {
	Key {
		code: KeyCode::Enter,
		modifiers: Modifiers::NONE,
	}
}

fn with_interaction(editor: &mut crate::Editor, f: impl FnOnce(&mut crate::overlay::OverlayManager, &mut crate::Editor)) {
	let mut interaction = editor.state.overlay_system.take_interaction();
	f(&mut interaction, editor);
	editor.state.overlay_system.restore_interaction(interaction);
}

fn palette_input_view(editor: &crate::Editor) -> crate::ViewId {
	editor
		.state
		.overlay_system
		.interaction()
		.active()
		.map(|active| active.session.input)
		.expect("command palette input should exist")
}

fn palette_set_input(editor: &mut crate::Editor, text: &str, cursor: usize) {
	let input = palette_input_view(editor);
	let buffer = editor.state.core.buffers.get_buffer_mut(input).expect("palette input buffer should exist");
	buffer.reset_content(text);
	buffer.set_cursor_and_selection(cursor, Selection::point(cursor));
	with_interaction(editor, |interaction, ed| {
		interaction.on_buffer_edited(ed, input);
	});
}

fn palette_key(editor: &mut crate::Editor, key: Key) {
	with_interaction(editor, |interaction, ed| {
		let _ = interaction.handle_key(ed, key);
	});
}

fn palette_input_text(editor: &crate::Editor) -> String {
	let input = palette_input_view(editor);
	editor
		.state
		.core
		.buffers
		.get_buffer(input)
		.expect("palette input buffer should exist")
		.with_doc(|doc| doc.content().to_string())
		.trim_end_matches('\n')
		.to_string()
}

fn drain_queued_commands(editor: &mut crate::Editor) -> Vec<crate::command_queue::QueuedCommand> {
	editor.state.core.workspace.command_queue.drain().collect()
}

/// Must preserve manual selection intent while the user stays in one token context.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::update_completion_state`
/// * Failure symptom: Arrow-key selection snaps back to auto-selected items mid-typing.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_manual_selection_persists_within_token() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_key(&mut editor, key_down());
	let state = editor
		.overlays()
		.get::<crate::completion::CompletionState>()
		.expect("completion state should exist");
	let selected = state
		.selected_idx
		.and_then(|idx| state.items.get(idx))
		.map(|item| item.label.clone())
		.expect("selection should exist after Down");

	let query = selected
		.chars()
		.next()
		.map(|ch| ch.to_ascii_lowercase().to_string())
		.expect("selected label should be non-empty");
	palette_set_input(&mut editor, &query, query.chars().count());

	let state = editor
		.overlays()
		.get::<crate::completion::CompletionState>()
		.expect("completion state should exist");
	assert_eq!(state.selection_intent, crate::completion::SelectionIntent::Manual);
	let selected_after = state
		.selected_idx
		.and_then(|idx| state.items.get(idx))
		.map(|item| item.label.clone())
		.expect("selection should persist");
	assert_eq!(selected_after, selected);
}

/// Must reset selection intent to auto when completion token context changes.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::update_completion_state`
/// * Failure symptom: Selection from command token bleeds into argument-token completions.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_token_transition_resets_selection_intent() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_set_input(&mut editor, "theme", 5);
	palette_key(&mut editor, key_down());

	palette_set_input(&mut editor, "theme ", 6);
	let state = editor
		.overlays()
		.get::<crate::completion::CompletionState>()
		.expect("completion state should exist");
	assert_eq!(state.selection_intent, crate::completion::SelectionIntent::Auto);
	assert!(!state.items.is_empty(), "theme arg completion should have items");
	assert!(
		state.items.iter().all(|item| item.kind == crate::completion::CompletionKind::Theme),
		"theme arg completion should only emit theme items"
	);
}

/// Must preserve typed path prefix when applying file completion with tab.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::accept_tab_completion`
/// * Failure symptom: Tab completion rewrites to wrong directory or drops prefix segments.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_tab_preserves_path_prefix() {
	let tmp = tempfile::tempdir().expect("temp dir");
	let src_dir = tmp.path().join("src");
	std::fs::create_dir_all(&src_dir).expect("create src dir");
	std::fs::write(src_dir.join("main.rs"), "fn main() {}\n").expect("write file");

	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	let input = format!("open {}/ma", src_dir.display());
	palette_set_input(&mut editor, &input, input.chars().count());
	palette_key(&mut editor, key_tab());

	let text = palette_input_text(&editor);
	assert!(text.starts_with(&format!("open {}/", src_dir.display())));
	assert!(text.contains("main.rs"));
}

/// Must preserve closing quote semantics when tab-completing quoted file arguments.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::accept_tab_completion`
/// * Failure symptom: Quote balancing breaks and cursor lands outside intended token.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_tab_after_closing_quote_preserves_quote() {
	let tmp = tempfile::tempdir().expect("temp dir");
	let spaced_dir = tmp.path().join("My Folder");
	std::fs::create_dir_all(&spaced_dir).expect("create spaced dir");
	std::fs::write(spaced_dir.join("main.rs"), "fn main() {}\n").expect("write file");

	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	let input = format!("open \"{}/ma\"", spaced_dir.display());
	palette_set_input(&mut editor, &input, input.chars().count());
	palette_key(&mut editor, key_tab());

	let text = palette_input_text(&editor);
	assert!(text.contains('"'));
	assert!(text.ends_with("\" "), "tab should keep quote and leave one space after it");
}

/// Must avoid appending trailing space when tab-completing a command name that has no argument completion.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::should_append_space_after_completion`
/// * Failure symptom: Tab completion dismisses command list for terminal commands like `quit`.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_tab_terminal_command_does_not_append_space() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_set_input(&mut editor, "q", 1);
	palette_key(&mut editor, key_tab());

	let text = palette_input_text(&editor);
	assert_eq!(text, "quit");
}

/// Must append trailing space when tab-completing a command with argument completion flow.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::should_append_space_after_completion`
/// * Failure symptom: Tab completion does not transition into argument completion for commands like `theme`.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_tab_argument_command_appends_space() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_set_input(&mut editor, "the", 3);
	palette_key(&mut editor, key_tab());

	let text = palette_input_text(&editor);
	assert_eq!(text, "theme ");
}

/// Must treat Enter as completion (not commit) for unresolved command prefixes that expand to commands requiring arguments.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::enter_commit_decision`, `crate::overlay::controllers::command_palette::CommandPaletteOverlay::handle_picker_action`
/// * Failure symptom: Enter closes palette and emits missing-argument command error instead of applying completion text.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_enter_promotes_required_arg_command_completion() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_set_input(&mut editor, "the", 3);
	let state = editor.overlays_mut().get_or_default::<crate::completion::CompletionState>();
	state.active = true;
	state.items = vec![crate::completion::CompletionItem {
		label: "theme".to_string(),
		insert_text: "theme".to_string(),
		detail: None,
		filter_text: None,
		kind: crate::completion::CompletionKind::Command,
		match_indices: None,
		right: None,
		file: None,
	}];
	state.selected_idx = Some(0);
	state.selection_intent = crate::completion::SelectionIntent::Manual;

	let _ = futures::executor::block_on(editor.handle_key(key_enter()));

	assert_eq!(palette_input_text(&editor), "theme ");
	assert!(editor.state.overlay_system.interaction().is_open(), "enter completion should keep palette open");
	assert!(
		!editor.frame().deferred_work.has_overlay_commit(),
		"enter completion should not schedule commit"
	);
	assert!(drain_queued_commands(&mut editor).is_empty(), "enter completion should not queue commands");
}

/// Must treat Enter as completion (not commit) for exact command input when the command requires a missing argument.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::should_promote_enter_to_tab_completion`, `crate::overlay::controllers::command_palette::CommandPaletteOverlay::enter_commit_decision`
/// * Failure symptom: Enter commits `theme` without args and triggers required-argument errors instead of transitioning to argument completion.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_enter_promotes_exact_required_arg_command_completion() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_set_input(&mut editor, "theme", 5);
	let _ = futures::executor::block_on(editor.handle_key(key_enter()));

	assert_eq!(palette_input_text(&editor), "theme ");
	assert!(editor.state.overlay_system.interaction().is_open(), "enter completion should keep palette open");
	assert!(
		!editor.frame().deferred_work.has_overlay_commit(),
		"enter completion should not schedule commit"
	);
	assert!(drain_queued_commands(&mut editor).is_empty(), "enter completion should not queue commands");
}

/// Must commit selected theme completion as the first argument when command input is `theme `.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::should_apply_selected_argument_on_commit`, `crate::overlay::controllers::command_palette::CommandPaletteOverlay::on_commit`
/// * Failure symptom: Committing palette with selected theme runs `theme` without args and raises missing-argument errors.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_commit_theme_with_selected_completion_supplies_argument() {
	let theme_name = xeno_registry::themes::THEMES
		.snapshot_guard()
		.iter_refs()
		.next()
		.map(|theme| theme.name_str().to_string())
		.expect("themes registry should contain at least one theme");

	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_set_input(&mut editor, "theme ", 6);
	let state = editor.overlays_mut().get_or_default::<crate::completion::CompletionState>();
	state.active = true;
	state.items = vec![crate::completion::CompletionItem {
		label: theme_name.clone(),
		insert_text: theme_name.clone(),
		detail: None,
		filter_text: None,
		kind: crate::completion::CompletionKind::Theme,
		match_indices: None,
		right: None,
		file: None,
	}];
	state.selected_idx = Some(0);
	state.selection_intent = crate::completion::SelectionIntent::Manual;

	with_interaction(&mut editor, |interaction, ed| {
		futures::executor::block_on(interaction.commit(ed));
	});

	let commands = drain_queued_commands(&mut editor);
	assert_eq!(commands.len(), 1);
	assert_eq!(commands[0].name, "theme");
	assert_eq!(commands[0].args, vec![theme_name]);
}

/// Must rank recently used commands first for empty command query completion.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::build_command_items`, `crate::overlay::OverlayContext::record_command_usage`
/// * Failure symptom: Command palette ignores recency and shows low-signal default ordering.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_usage_recency_orders_empty_query() {
	let cmd_name = xeno_registry::commands::COMMANDS
		.snapshot_guard()
		.iter_refs()
		.next()
		.map(|cmd| cmd.name_str().to_string())
		.expect("registry should have at least one command");

	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_set_input(&mut editor, &cmd_name, cmd_name.chars().count());
	with_interaction(&mut editor, |interaction, ed| {
		futures::executor::block_on(interaction.commit(ed));
	});

	assert!(editor.open_command_palette());
	palette_set_input(&mut editor, "", 0);

	let state = editor
		.overlays()
		.get::<crate::completion::CompletionState>()
		.expect("completion state should exist");
	let first = state
		.items
		.first()
		.map(|item| item.label.as_str())
		.expect("completion list should be non-empty");
	assert_eq!(first, cmd_name);
}

/// Must prefer exact typed command resolution over selected completion when both are available.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::resolve_command_name_for_commit`
/// * Failure symptom: Enter executes a different command than the exact text the user entered.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_commit_prefers_typed_resolved_command() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_set_input(&mut editor, "write", 5);
	let state = editor.overlays_mut().get_or_default::<crate::completion::CompletionState>();
	state.active = true;
	state.items = vec![crate::completion::CompletionItem {
		label: "quit".to_string(),
		insert_text: "quit".to_string(),
		detail: None,
		filter_text: None,
		kind: crate::completion::CompletionKind::Command,
		match_indices: None,
		right: None,
		file: None,
	}];
	state.selected_idx = Some(0);
	state.selection_intent = crate::completion::SelectionIntent::Manual;

	with_interaction(&mut editor, |interaction, ed| {
		futures::executor::block_on(interaction.commit(ed));
	});

	let commands = drain_queued_commands(&mut editor);
	assert_eq!(commands.len(), 1);
	assert_eq!(commands[0].name, "write");
}

/// Must fall back to selected command completion when typed command is unresolved.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::resolve_command_name_for_commit`
/// * Failure symptom: Enter on incomplete command emits unknown-command error instead of accepting active completion.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_commit_falls_back_to_selected_command_when_unresolved() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	palette_set_input(&mut editor, "wri", 3);
	let state = editor.overlays_mut().get_or_default::<crate::completion::CompletionState>();
	state.active = true;
	state.items = vec![crate::completion::CompletionItem {
		label: "write".to_string(),
		insert_text: "write".to_string(),
		detail: None,
		filter_text: None,
		kind: crate::completion::CompletionKind::Command,
		match_indices: None,
		right: None,
		file: None,
	}];
	state.selected_idx = Some(0);
	state.selection_intent = crate::completion::SelectionIntent::Manual;

	with_interaction(&mut editor, |interaction, ed| {
		futures::executor::block_on(interaction.commit(ed));
	});

	let commands = drain_queued_commands(&mut editor);
	assert_eq!(commands.len(), 1);
	assert_eq!(commands[0].name, "write");
	let notifications = editor.take_notification_render_items();
	assert!(notifications.is_empty(), "fallback completion should avoid unknown-command notifications");
}

/// Must preserve quoted argument token boundaries on command commit.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::tokenize`, `crate::overlay::controllers::command_palette::CommandPaletteOverlay::on_commit`
/// * Failure symptom: Quoted filenames are split into multiple args.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_commit_preserves_quoted_argument() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	let input = "edit \"my file.rs\"";
	palette_set_input(&mut editor, input, input.chars().count());
	with_interaction(&mut editor, |interaction, ed| {
		futures::executor::block_on(interaction.commit(ed));
	});

	let commands = drain_queued_commands(&mut editor);
	assert_eq!(commands.len(), 1);
	assert_eq!(commands[0].name, "edit");
	assert_eq!(commands[0].args, vec!["my file.rs".to_string()]);
}

/// Must preserve quoted snippet-body arguments on command commit.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::tokenize`, `crate::overlay::controllers::command_palette::CommandPaletteOverlay::on_commit`
/// * Failure symptom: Snippet placeholders are truncated or split across args.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_commit_preserves_quoted_snippet_body_argument() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	let input = "snippet \"${1:x} ${2:y}\"";
	palette_set_input(&mut editor, input, input.chars().count());
	with_interaction(&mut editor, |interaction, ed| {
		futures::executor::block_on(interaction.commit(ed));
	});

	let commands = drain_queued_commands(&mut editor);
	assert_eq!(commands.len(), 1);
	assert_eq!(commands[0].name, "snippet");
	assert_eq!(commands[0].args, vec!["${1:x} ${2:y}".to_string()]);
}

/// Must provide snippet-name completions only for `@`-prefixed snippet query token.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::build_items_for_token`, `crate::overlay::controllers::command_palette::CommandPaletteOverlay::build_snippet_items`
/// * Failure symptom: Snippet command cannot discover named registry snippets.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_snippet_name_completion_with_at_query() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	let input = "snippet @f";
	palette_set_input(&mut editor, input, input.chars().count());

	let state = editor
		.overlays()
		.get::<crate::completion::CompletionState>()
		.expect("completion state should exist");
	assert!(state.active, "snippet completion should be active for @ query");
	assert!(
		state
			.items
			.iter()
			.any(|item| item.kind == crate::completion::CompletionKind::Snippet && item.label == "@fori"),
		"snippet name completion should include @fori"
	);
}

/// Must suppress snippet-name completions when snippet command uses inline body syntax.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::build_items_for_token`
/// * Failure symptom: Inline snippet editing is polluted by irrelevant snippet-name entries.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_snippet_inline_body_has_no_name_completions() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	let input = "snippet ${1:";
	palette_set_input(&mut editor, input, input.chars().count());

	let state = editor
		.overlays()
		.get::<crate::completion::CompletionState>()
		.expect("completion state should exist");
	assert!(!state.active);
	assert!(state.items.is_empty());
}

/// Must hide completion UI and keep input unchanged when query has no matches.
///
/// * Enforced in: `crate::overlay::controllers::command_palette::CommandPaletteOverlay::update_completion_state`, `crate::overlay::controllers::command_palette::CommandPaletteOverlay::accept_tab_completion`
/// * Failure symptom: Empty-match tab inserts stale completion text.
#[cfg_attr(test, test)]
pub(crate) fn test_palette_no_matches_hides_results_and_tab_noops() {
	let mut editor = crate::Editor::new_scratch();
	editor.handle_window_resize(120, 40);
	assert!(editor.open_command_palette());

	let query = "zzzzzzzzzz";
	palette_set_input(&mut editor, query, query.chars().count());

	let state = editor
		.overlays()
		.get::<crate::completion::CompletionState>()
		.expect("completion state should exist");
	assert!(!state.active);
	assert!(state.items.is_empty());

	let before = palette_input_text(&editor);
	palette_key(&mut editor, key_tab());
	let after = palette_input_text(&editor);
	assert_eq!(after, before, "tab should not mutate input when there are no results");
}
