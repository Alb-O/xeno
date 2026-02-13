use super::*;

#[test]
fn ime_preedit_label_truncates_long_content() {
	assert_eq!(ime_preedit_label(None), "-");
	assert_eq!(ime_preedit_label(Some("short")), "short");
	assert_eq!(ime_preedit_label(Some("abcdefghijklmnopqrstuvwxyz")), "abcdefghijklmnopqrstuvwx...");
}

#[test]
fn merge_render_lines_preserves_gutter_then_text_order() {
	let style = Style::default();
	let gutter = vec![RenderLine::from(vec![RenderSpan::styled(" 1 ", style)])];
	let text = vec![RenderLine::from(vec![RenderSpan::styled("alpha", style)])];

	let rows = merge_render_lines(gutter, text);
	assert_eq!(rows.len(), 1);
	assert_eq!(rows[0].spans.len(), 2);
	assert_eq!(rows[0].spans[0].content.as_ref(), " 1 ");
	assert_eq!(rows[0].spans[1].content.as_ref(), "alpha");
}
