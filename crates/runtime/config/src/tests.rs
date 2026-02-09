use super::*;

#[test]
fn test_load_theme_file() {
	let dir = tempfile::tempdir().expect("tempdir");
	let file = dir.path().join("sample.kdl");
	std::fs::write(
		&file,
		r##"
name "sample"
variant "dark"

ui {
    bg "#000000"
    fg "#ffffff"
    gutter-fg "#888888"
    cursor-bg "#ffffff"
    cursor-fg "#000000"
    cursorline-bg "#111111"
    selection-bg "#222222"
    selection-fg "#ffffff"
    message-fg "#ffffff"
    command-input-fg "#ffffff"
}

mode {
    normal-bg "#111111"
    normal-fg "#ffffff"
    insert-bg "#222222"
    insert-fg "#ffffff"
    prefix-bg "#333333"
    prefix-fg "#ffffff"
    command-bg "#444444"
    command-fg "#ffffff"
}

semantic {
    error "#ff0000"
    warning "#ffaa00"
    success "#00ff00"
    info "#00aaff"
    hint "#8888ff"
    dim "#666666"
    link "#00aaff"
    match "#444444"
    accent "#ff00ff"
}

popup {
    bg "#111111"
    fg "#ffffff"
    border "#333333"
    title "#ffffff"
}
"##,
	)
	.expect("write theme file");

	let parsed = load_theme_file(&file).expect("parse theme file");
	assert_eq!(parsed.name, "sample");
	assert_eq!(parsed.source_path.as_deref(), Some(file.as_path()));
}
