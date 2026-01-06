//! Document rendering logic for the editor.
//!
//! This module handles rendering of buffers in split views, including
//! separator styling and junction glyphs.

/// Line wrapping calculations for soft-wrapped text.
mod wrapping;

use std::time::{Duration, SystemTime};

use xeno_tui::animation::Animatable;
use xeno_tui::layout::{Constraint, Direction, Layout, Rect};
use xeno_tui::style::{Color, Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::menu::Menu;
use xeno_tui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, StatefulWidget};

use super::buffer::{BufferRenderContext, ensure_buffer_cursor_visible};
use crate::Editor;
use crate::buffer::{BufferView, SplitDirection};
use crate::editor::FocusTarget;
use crate::test_events::SeparatorAnimationEvent;

/// Per-layer rendering data: (layer_index, layer_area, view_areas, separators).
type LayerRenderData = (
	usize,
	Rect,
	Vec<(BufferView, Rect)>,
	Vec<(SplitDirection, u8, Rect)>,
);

/// Extracts RGB components from a color, if it's an RGB color.
fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
	match color {
		Color::Rgb(r, g, b) => Some((r, g, b)),
		_ => None,
	}
}

/// Precomputed separator colors and state for efficient style lookups.
struct SeparatorStyle {
	/// Rectangle of the currently hovered separator.
	hovered_rect: Option<Rect>,
	/// Rectangle of the separator being dragged.
	dragging_rect: Option<Rect>,
	/// Rectangle of the separator being animated.
	anim_rect: Option<Rect>,
	/// Animation intensity (0.0 to 1.0) for hover transitions.
	anim_intensity: f32,
	/// Base colors per visual priority level (index = priority).
	base_bg: [Color; 2],
	/// Foreground colors per visual priority level.
	base_fg: [Color; 2],
	/// Foreground color for hovered separators.
	hover_fg: Color,
	/// Background color for hovered separators.
	hover_bg: Color,
	/// Foreground color for actively dragged separators.
	drag_fg: Color,
	/// Background color for actively dragged separators.
	drag_bg: Color,
}

impl SeparatorStyle {
	/// Creates a new separator style from the current editor state.
	fn new(editor: &Editor, doc_area: Rect) -> Self {
		Self {
			hovered_rect: editor.layout.hovered_separator.map(|(_, rect)| rect),
			dragging_rect: editor.layout.drag_state().and_then(|ds| {
				editor
					.layout
					.separator_rect(&editor.base_window().layout, doc_area, &ds.id)
			}),
			anim_rect: editor.layout.animation_rect(),
			anim_intensity: editor.layout.animation_intensity(),
			base_bg: [editor.theme.colors.ui.bg, editor.theme.colors.popup.bg],
			base_fg: [
				editor.theme.colors.ui.gutter_fg,
				editor.theme.colors.popup.fg,
			],
			hover_fg: editor.theme.colors.ui.cursor_fg,
			hover_bg: editor.theme.colors.ui.selection_bg,
			drag_fg: editor.theme.colors.ui.bg,
			drag_bg: editor.theme.colors.ui.fg,
		}
	}

	/// Returns the style for a separator at the given rectangle and priority.
	fn for_rect(&self, rect: Rect, priority: u8) -> Style {
		let is_dragging = self.dragging_rect == Some(rect);
		let is_animating = self.anim_rect == Some(rect);
		let is_hovered = self.hovered_rect == Some(rect);

		let idx = (priority as usize).min(self.base_bg.len() - 1);
		let normal_fg = self.base_fg[idx];
		let normal_bg = self.base_bg[idx];

		if is_dragging {
			Style::default().fg(self.drag_fg).bg(self.drag_bg)
		} else if is_animating {
			let fg = normal_fg.lerp(&self.hover_fg, self.anim_intensity);
			let bg = normal_bg.lerp(&self.hover_bg, self.anim_intensity);
			if let (Some(fg_rgb), Some(bg_rgb)) = (color_to_rgb(fg), color_to_rgb(bg)) {
				SeparatorAnimationEvent::frame(self.anim_intensity, fg_rgb, bg_rgb);
			}
			Style::default().fg(fg).bg(bg)
		} else if is_hovered {
			Style::default().fg(self.hover_fg).bg(self.hover_bg)
		} else {
			Style::default().fg(normal_fg).bg(normal_bg)
		}
	}

	/// Returns the style for a junction at the given position.
	///
	/// Checks if the position lies on a hovered/dragged/animated separator to maintain
	/// continuous highlight across junctions.
	fn for_junction(&self, x: u16, y: u16, priority: u8) -> Style {
		let point_on_rect = |rect: Rect| -> bool {
			x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
		};

		let idx = (priority as usize).min(self.base_bg.len() - 1);
		let normal_fg = self.base_fg[idx];
		let normal_bg = self.base_bg[idx];

		if self.dragging_rect.is_some_and(point_on_rect) {
			Style::default().fg(self.drag_fg).bg(self.drag_bg)
		} else if self.anim_rect.is_some_and(point_on_rect) {
			let fg = normal_fg.lerp(&self.hover_fg, self.anim_intensity);
			let bg = normal_bg.lerp(&self.hover_bg, self.anim_intensity);
			Style::default().fg(fg).bg(bg)
		} else if self.hovered_rect.is_some_and(point_on_rect) {
			Style::default().fg(self.hover_fg).bg(self.hover_bg)
		} else {
			Style::default().fg(normal_fg).bg(normal_bg)
		}
	}
}

/// Returns the box-drawing junction glyph for the given connectivity.
///
/// Connectivity is encoded as a 4-bit mask: up (0x1), down (0x2), left (0x4), right (0x8).
fn junction_glyph(connectivity: u8) -> char {
	match connectivity {
		0b1111 => '┼',
		0b1011 => '├',
		0b0111 => '┤',
		0b1110 => '┬',
		0b1101 => '┴',
		0b0011 => '│',
		0b1100 => '─',
		_ => '┼',
	}
}

/// Clamps a rectangle to a bounding area, returning the intersection.
fn clamp_rect(rect: Rect, bounds: Rect) -> Option<Rect> {
	let x1 = rect.x.max(bounds.x);
	let y1 = rect.y.max(bounds.y);
	let x2 = rect.right().min(bounds.right());
	let y2 = rect.bottom().min(bounds.bottom());

	if x2 <= x1 || y2 <= y1 {
		return None;
	}

	Some(Rect {
		x: x1,
		y: y1,
		width: x2.saturating_sub(x1),
		height: y2.saturating_sub(y1),
	})
}

impl Editor {
	/// Renders the complete editor frame.
	///
	/// This is the main rendering entry point that orchestrates all UI elements:
	/// - Document content with cursor and selections (including splits)
	/// - UI panels (if any)
	/// - Command/message line
	/// - Status line
	/// - Notifications
	///
	/// # Parameters
	/// - `frame`: The xeno_tui frame to render into
	pub fn render(&mut self, frame: &mut xeno_tui::Frame) {
		let now = SystemTime::now();
		let delta = now
			.duration_since(self.last_tick)
			.unwrap_or(Duration::from_millis(16));
		self.last_tick = now;
		self.notifications.tick(delta);

		// Update style overlays to reflect current cursor position.
		// This must happen at render time (not tick time) to handle
		// mouse clicks and other events that modify cursor after tick.
		self.update_style_overlays();

		let use_block_cursor = true;

		let area = frame.area();
		self.window_width = Some(area.width);
		self.window_height = Some(area.height);

		frame.render_widget(Clear, area);

		let bg_block = Block::default().style(Style::default().bg(self.theme.colors.ui.bg));
		frame.render_widget(bg_block, area);

		let chunks = Layout::default()
			.direction(Direction::Vertical)
			.constraints([
				Constraint::Length(1),
				Constraint::Min(1),
				Constraint::Length(1),
			])
			.split(area);

		let menu_area = chunks[0];
		let main_area = chunks[1];
		let status_area = chunks[2];

		let mut ui = std::mem::take(&mut self.ui);
		let dock_layout = ui.compute_layout(main_area);
		let doc_area = dock_layout.doc_area;
		self.doc_area = Some(doc_area);

		let doc_focused = ui.focus.focused().is_editor();

		// Render all buffers in the layout
		self.render_split_buffers(frame, doc_area, use_block_cursor && doc_focused);
		self.render_floating_windows(frame, use_block_cursor && doc_focused);

		if let Some(cursor_pos) = ui.render_panels(self, frame, &dock_layout, self.theme) {
			frame.set_cursor_position(cursor_pos);
		}
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;

		let menu_bg = Block::default().style(Style::default().bg(self.theme.colors.popup.bg));
		frame.render_widget(menu_bg, menu_area);
		Menu::new()
			.style(
				Style::default()
					.fg(self.theme.colors.popup.fg)
					.bg(self.theme.colors.popup.bg),
			)
			.highlight_style(
				Style::default()
					.fg(self.theme.colors.ui.selection_fg)
					.bg(self.theme.colors.ui.selection_bg),
			)
			.render(menu_area, frame.buffer_mut(), &mut self.menu);

		let status_bg = Block::default().style(Style::default().bg(self.theme.colors.popup.bg));
		frame.render_widget(status_bg, status_area);
		frame.render_widget(self.render_status_line(), status_area);

		let mut notifications_area = doc_area;
		notifications_area.height = notifications_area.height.saturating_sub(1);
		notifications_area.width = notifications_area.width.saturating_sub(1);
		self.notifications
			.render(notifications_area, frame.buffer_mut());

		self.render_whichkey_hud(frame, doc_area);
	}

	/// Renders all views across all layout layers.
	///
	/// Layer 0 is rendered first (base), then overlay layers on top.
	/// Each layer's views and separators are rendered together before moving to the next layer.
	fn render_split_buffers(
		&mut self,
		frame: &mut xeno_tui::Frame,
		doc_area: Rect,
		use_block_cursor: bool,
	) {
		let focused_view = self.focused_view();
		let base_layout = &self.base_window().layout;

		let layer_count = self.layout.layer_count();
		let mut layer_data: Vec<LayerRenderData> = Vec::new();

		for layer_idx in 0..layer_count {
			if self.layout.layer(base_layout, layer_idx).is_some() {
				let layer_area = self.layout.layer_area(layer_idx, doc_area);
				let view_areas =
					self.layout
						.compute_view_areas_for_layer(base_layout, layer_idx, layer_area);
				let separators =
					self.layout
						.separator_positions_for_layer(base_layout, layer_idx, layer_area);
				layer_data.push((layer_idx, layer_area, view_areas, separators));
			}
		}

		// Ensure cursor is visible for all buffers
		for (_, _, view_areas, _) in &layer_data {
			for (buffer_id, area) in view_areas {
				let tab_width = self.tab_width_for(*buffer_id);
				if let Some(buffer) = self.get_buffer_mut(*buffer_id) {
					ensure_buffer_cursor_visible(buffer, *area, tab_width);
				}
			}
		}

		if self.layout.hovered_separator.is_none()
			&& self.layout.separator_under_mouse.is_some()
			&& !self.layout.is_mouse_fast()
		{
			let old_hover = self.layout.hovered_separator.take();
			self.layout.hovered_separator = self.layout.separator_under_mouse;
			if old_hover != self.layout.hovered_separator {
				self.layout
					.update_hover_animation(old_hover, self.layout.hovered_separator);
				self.needs_redraw = true;
			}
		}
		if self.layout.animation_needs_redraw() {
			self.needs_redraw = true;
		}

		let sep_style = SeparatorStyle::new(self, doc_area);

		let ctx = BufferRenderContext {
			theme: self.theme,
			language_loader: &self.language_loader,
			style_overlays: &self.style_overlays,
		};

		for (_, _, view_areas, separators) in &layer_data {
			for (buffer_id, area) in view_areas {
				let is_focused = *buffer_id == focused_view;
				let tab_width = self.tab_width_for(*buffer_id);
				if let Some(buffer) = self.get_buffer(*buffer_id) {
					let result =
						ctx.render_buffer(buffer, *area, use_block_cursor, is_focused, tab_width);
					frame.render_widget(result.widget, *area);
				}
			}

			for (direction, priority, sep_rect) in separators {
				let style = sep_style.for_rect(*sep_rect, *priority);
				let lines: Vec<Line> = match direction {
					SplitDirection::Horizontal => (0..sep_rect.height)
						.map(|_| Line::from(Span::styled("\u{2502}", style)))
						.collect(),
					SplitDirection::Vertical => vec![Line::from(Span::styled(
						"\u{2500}".repeat(sep_rect.width as usize),
						style,
					))],
				};
				frame.render_widget(Paragraph::new(lines), *sep_rect);
			}

			self.render_separator_junctions(frame, separators, &sep_style);
		}
	}

	/// Renders floating windows above the base layout.
	fn render_floating_windows(&mut self, frame: &mut xeno_tui::Frame, use_block_cursor: bool) {
		let bounds = frame.area();
		let focused = match &self.focus {
			FocusTarget::Buffer { window, buffer } => Some((*window, *buffer)),
			_ => None,
		};

		let floating_windows: Vec<_> = self
			.windows
			.floating_windows()
			.map(|(id, window)| (id, window.clone()))
			.collect();
		for (_, window) in &floating_windows {
			let Some(rect) = clamp_rect(window.rect, bounds) else {
				continue;
			};
			let content_area = if window.style.border {
				Rect {
					x: rect.x.saturating_add(1),
					y: rect.y.saturating_add(1),
					width: rect.width.saturating_sub(2),
					height: rect.height.saturating_sub(2),
				}
			} else {
				rect
			};

			if content_area.width == 0 || content_area.height == 0 {
				continue;
			}

			let tab_width = self.tab_width_for(window.buffer);
			if let Some(buffer) = self.get_buffer_mut(window.buffer) {
				ensure_buffer_cursor_visible(buffer, content_area, tab_width);
			}
		}

		let ctx = BufferRenderContext {
			theme: self.theme,
			language_loader: &self.language_loader,
			style_overlays: &self.style_overlays,
		};

		for (window_id, window) in floating_windows {
			let Some(rect) = clamp_rect(window.rect, bounds) else {
				continue;
			};

			if window.style.shadow {
				let shadow_rect = Rect {
					x: rect.x.saturating_add(1),
					y: rect.y.saturating_add(1),
					width: rect.width,
					height: rect.height,
				};
				if let Some(shadow) = clamp_rect(shadow_rect, bounds) {
					let shadow_block =
						Block::default().style(Style::default().bg(self.theme.colors.ui.bg));
					frame.render_widget(shadow_block, shadow);
				}
			}

			frame.render_widget(Clear, rect);

			let mut block = Block::default()
				.style(Style::default().bg(self.theme.colors.popup.bg))
				.padding(window.style.padding);
			if window.style.border {
				block = block
					.borders(Borders::ALL)
					.border_type(window.style.border_type)
					.border_style(Style::default().fg(self.theme.colors.popup.fg));
				if let Some(title) = &window.style.title {
					block = block.title(title.as_str());
				}
			}

			let content_area = block.inner(rect);

			frame.render_widget(block, rect);

			if content_area.width == 0 || content_area.height == 0 {
				continue;
			}

			if let Some(buffer) = self.get_buffer(window.buffer) {
				let is_focused = focused
					.map(|(win, buf)| win == window_id && buf == window.buffer)
					.unwrap_or(false);
				let tab_width = self.tab_width_for(window.buffer);
				let result = ctx.render_buffer_with_gutter(
					buffer,
					content_area,
					use_block_cursor,
					is_focused,
					window.gutter,
					tab_width,
				);
				frame.render_widget(result.widget, content_area);
			}
		}
	}

	/// Renders junction glyphs where separators intersect within a layer.
	fn render_separator_junctions(
		&self,
		frame: &mut xeno_tui::Frame,
		separators: &[(SplitDirection, u8, Rect)],
		sep_style: &SeparatorStyle,
	) {
		use std::collections::HashMap;

		type JunctionState = (bool, bool, bool, bool, u8);
		type JunctionMap = HashMap<(u16, u16), JunctionState>;

		// SplitDirection::Vertical = stacked = horizontal line; Horizontal = side-by-side = vertical line
		let h_seps: Vec<_> = separators
			.iter()
			.filter(|(d, _, _)| *d == SplitDirection::Vertical)
			.collect();
		let v_seps: Vec<_> = separators
			.iter()
			.filter(|(d, _, _)| *d == SplitDirection::Horizontal)
			.collect();

		// (x, y) -> (has_up, has_down, has_left, has_right, priority)
		let mut all_junctions: JunctionMap = HashMap::new();

		for (_, v_prio, v_rect) in &v_seps {
			let x = v_rect.x;

			for (_, h_prio, h_rect) in &h_seps {
				let y = h_rect.y;

				let at_left_edge = x + 1 == h_rect.x;
				let at_right_edge = x == h_rect.right();
				let x_overlaps = x >= h_rect.x && x < h_rect.right();

				let touches_above = y >= v_rect.y && y < v_rect.bottom();
				let adjacent_above = y == v_rect.bottom();
				let adjacent_below = y + 1 == v_rect.y;
				let touches_below = y + 1 >= v_rect.y && y + 1 < v_rect.bottom();
				let within = x_overlaps && touches_above;

				let dominated_above = x_overlaps && adjacent_above;
				let dominated_below = x_overlaps && adjacent_below;

				if !(at_left_edge
					|| at_right_edge
					|| within || dominated_above
					|| dominated_below
					|| (x_overlaps && touches_below))
				{
					continue;
				}

				if touches_above || touches_below || within || dominated_above || dominated_below {
					let entry = all_junctions
						.entry((x, y))
						.or_insert((false, false, false, false, 0));
					if within {
						entry.0 |= y > v_rect.y;
						entry.1 |= y < v_rect.bottom().saturating_sub(1);
						entry.2 |= x > h_rect.x;
						entry.3 |= x < h_rect.right().saturating_sub(1);
					} else if dominated_above {
						entry.0 = true;
						entry.2 |= x > h_rect.x;
						entry.3 |= x < h_rect.right().saturating_sub(1);
					} else if dominated_below {
						entry.1 = true;
						entry.2 |= x > h_rect.x;
						entry.3 |= x < h_rect.right().saturating_sub(1);
					} else if x_overlaps {
						entry.0 |= touches_above;
						entry.1 |= touches_below;
						entry.2 |= x > h_rect.x;
						entry.3 |= x < h_rect.right().saturating_sub(1);
					} else {
						entry.0 |= touches_above;
						entry.1 |= touches_below;
						entry.2 |= at_right_edge;
						entry.3 |= at_left_edge;
					}
					entry.4 = entry.4.max(*v_prio).max(*h_prio);
				}
			}
		}

		let buf = frame.buffer_mut();
		for ((x, y), (has_up, has_down, has_left, has_right, priority)) in all_junctions {
			let connectivity = (has_up as u8)
				| ((has_down as u8) << 1)
				| ((has_left as u8) << 2)
				| ((has_right as u8) << 3);

			if connectivity == 0b0011 {
				continue;
			}

			let glyph = junction_glyph(connectivity);
			let style = sep_style.for_junction(x, y, priority);

			if let Some(cell) = buf.cell_mut((x, y)) {
				cell.set_char(glyph);
				cell.set_style(style);
			}
		}
	}

	/// Renders the which-key HUD when there are pending keys.
	fn render_whichkey_hud(&self, frame: &mut xeno_tui::Frame, doc_area: Rect) {
		use xeno_core::get_keymap_registry;
		use xeno_core::keymap_registry::ContinuationKind;
		use xeno_registry::{BindingMode, find_prefix};
		use xeno_tui::widgets::keytree::{KeyTree, KeyTreeNode};

		let pending_keys = self.buffer().input.pending_keys();
		if pending_keys.is_empty() {
			return;
		}

		let binding_mode = match self.buffer().input.mode() {
			xeno_base::Mode::Normal => BindingMode::Normal,
			_ => return,
		};

		let continuations =
			get_keymap_registry().continuations_with_kind(binding_mode, pending_keys);
		if continuations.is_empty() {
			return;
		}

		let key_strs: Vec<String> = pending_keys.iter().map(|k| format!("{k}")).collect();
		let (root, ancestors) = if key_strs.len() <= 1 {
			(key_strs.first().cloned().unwrap_or_default(), vec![])
		} else {
			let root = key_strs[0].clone();
			let mut ancestors = Vec::new();
			let mut prefix_so_far = root.clone();

			for key in &key_strs[1..] {
				prefix_so_far = format!("{prefix_so_far} {key}");
				let desc = find_prefix(binding_mode, &prefix_so_far)
					.map(|p| p.description)
					.unwrap_or("");
				ancestors.push(KeyTreeNode::new(key.clone(), desc));
			}
			(root, ancestors)
		};

		let prefix_key = key_strs.join(" ");
		let root_desc = find_prefix(binding_mode, &key_strs[0]).map(|p| p.description);

		let children: Vec<KeyTreeNode<'_>> = continuations
			.iter()
			.map(|cont| {
				let key = format!("{}", cont.key);
				match cont.kind {
					ContinuationKind::Branch => {
						let sub_prefix = if prefix_key.is_empty() {
							key.clone()
						} else {
							format!("{prefix_key} {key}")
						};
						let desc = find_prefix(binding_mode, &sub_prefix)
							.map(|p| p.description)
							.unwrap_or("");
						KeyTreeNode::with_suffix(key, desc, "…")
					}
					ContinuationKind::Leaf => {
						let desc = cont.value.map_or("", |e| {
							if !e.short_desc.is_empty() {
								e.short_desc
							} else if !e.description.is_empty() {
								e.description
							} else {
								e.action_name
							}
						});
						KeyTreeNode::new(key, desc)
					}
				}
			})
			.collect();

		let ancestor_lines = ancestors.len() as u16;
		let content_height = (children.len() as u16 + ancestor_lines + 2).clamp(3, 14);
		let width = 32u16.min(doc_area.width.saturating_sub(4));
		let height = content_height + 2;
		let hud_area = Rect {
			x: doc_area.x + doc_area.width.saturating_sub(width + 2),
			y: doc_area.y + doc_area.height.saturating_sub(height + 2),
			width,
			height,
		};

		let block = Block::default()
			.style(
				Style::default()
					.bg(self.theme.colors.popup.bg)
					.fg(self.theme.colors.popup.fg),
			)
			.borders(Borders::ALL)
			.border_type(BorderType::Stripe)
			.border_style(Style::default().fg(self.theme.colors.status.warning_fg))
			.padding(Padding::horizontal(1));

		let inner = block.inner(hud_area);
		frame.render_widget(Clear, hud_area);
		frame.render_widget(block, hud_area);

		let mut tree = KeyTree::new(root, children)
			.ancestors(ancestors)
			.ancestor_style(Style::default().fg(self.theme.colors.ui.gutter_fg))
			.key_style(
				Style::default()
					.fg(self.theme.colors.status.warning_fg)
					.add_modifier(Modifier::BOLD),
			)
			.desc_style(Style::default().fg(self.theme.colors.popup.fg))
			.suffix_style(Style::default().fg(self.theme.colors.ui.gutter_fg))
			.line_style(Style::default().fg(self.theme.colors.ui.gutter_fg));

		if let Some(desc) = root_desc {
			tree = tree.root_desc(desc);
		}

		frame.render_widget(tree, inner);
	}
}
