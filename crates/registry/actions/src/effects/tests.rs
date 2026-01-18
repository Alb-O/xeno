use super::*;

#[test]
fn test_effects_composition() {
	let sel = Selection::single(10, 10);
	let effects = ActionEffects::motion(sel.clone()).with(AppEffect::SetMode(Mode::Insert));

	assert_eq!(effects.len(), 2);
	assert!(matches!(
		effects.as_slice()[0],
		Effect::View(ViewEffect::SetSelection(_))
	));
	assert!(matches!(
		effects.as_slice()[1],
		Effect::App(AppEffect::SetMode(Mode::Insert))
	));
}

#[test]
fn test_effects_ok_is_empty() {
	let effects = ActionEffects::ok();
	assert!(effects.is_empty());
}

#[test]
fn test_from_effect() {
	let effects: ActionEffects = AppEffect::SetMode(Mode::Normal).into();
	assert_eq!(effects.len(), 1);
}

#[test]
fn test_nested_view_effect() {
	let effect: Effect = ViewEffect::SetCursor(CharIdx::from(42usize)).into();
	assert!(matches!(effect, Effect::View(ViewEffect::SetCursor(_))));
}

#[test]
fn test_nested_edit_effect() {
	let effect: Effect = EditEffect::Paste { before: true }.into();
	assert!(matches!(
		effect,
		Effect::Edit(EditEffect::Paste { before: true })
	));
}

#[test]
fn test_nested_ui_effect() {
	let effect: Effect = UiEffect::OpenPalette.into();
	assert!(matches!(effect, Effect::Ui(UiEffect::OpenPalette)));
}

#[test]
fn test_nested_app_effect() {
	let effect: Effect = AppEffect::Quit { force: true }.into();
	assert!(matches!(
		effect,
		Effect::App(AppEffect::Quit { force: true })
	));
}

#[test]
fn test_from_implementations() {
	let effect: Effect = Selection::point(CharIdx::from(10usize)).into();
	assert!(matches!(effect, Effect::View(ViewEffect::SetSelection(_))));

	let effect: Effect = Mode::Insert.into();
	assert!(matches!(
		effect,
		Effect::App(AppEffect::SetMode(Mode::Insert))
	));

	let notification: Notification = xeno_registry_notifications::keys::UNDO.into();
	let effect: Effect = notification.into();
	assert!(matches!(effect, Effect::Ui(UiEffect::Notify(_))));
}
