use xeno_tui::layout::{Position, Rect};

pub type ZIndex = i16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
	Background,
	Document,
	InfoPopups,
	Panels,
	CompletionPopup,
	OverlayLayers,
	StatusLine,
	Notifications,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceOp {
	Background,
	Document,
	InfoPopups,
	Panels,
	CompletionPopup,
	OverlayLayers,
	StatusLine,
	Notifications,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surface {
	pub id: SurfaceId,
	pub kind: SurfaceKind,
	pub z: ZIndex,
	pub area: Rect,
	pub op: SurfaceOp,
	pub accepts_mouse: bool,
}

#[derive(Debug, Clone)]
pub struct UiScene {
	pub screen: Rect,
	pub main_area: Rect,
	pub doc_area: Rect,
	pub status_area: Rect,
	pub surfaces: Vec<Surface>,
}

impl UiScene {
	pub fn sort_stable(&mut self) {
		self.surfaces.sort_by_key(|surface| (surface.z, surface.id.0));
	}

	pub fn hit_test(&self, x: u16, y: u16) -> Option<&Surface> {
		self.surfaces.iter().rev().find(|surface| {
			surface.accepts_mouse
				&& x >= surface.area.x
				&& x < surface.area.x.saturating_add(surface.area.width)
				&& y >= surface.area.y
				&& y < surface.area.y.saturating_add(surface.area.height)
		})
	}
}

#[derive(Debug, Default, Clone)]
pub struct SceneRenderResult {
	pub cursor: Option<Position>,
}

#[cfg(test)]
mod tests {
	use xeno_tui::layout::Rect;

	use super::{Surface, SurfaceId, SurfaceKind, SurfaceOp, UiScene};

	fn sample_scene() -> UiScene {
		UiScene {
			screen: Rect::new(0, 0, 100, 40),
			main_area: Rect::new(0, 0, 100, 39),
			doc_area: Rect::new(0, 0, 80, 39),
			status_area: Rect::new(0, 39, 100, 1),
			surfaces: vec![
				Surface {
					id: SurfaceId(10),
					kind: SurfaceKind::Document,
					z: 10,
					area: Rect::new(0, 0, 10, 10),
					op: SurfaceOp::Document,
					accepts_mouse: true,
				},
				Surface {
					id: SurfaceId(2),
					kind: SurfaceKind::Panels,
					z: 30,
					area: Rect::new(2, 2, 10, 10),
					op: SurfaceOp::Panels,
					accepts_mouse: true,
				},
				Surface {
					id: SurfaceId(1),
					kind: SurfaceKind::Background,
					z: 0,
					area: Rect::new(0, 0, 100, 40),
					op: SurfaceOp::Background,
					accepts_mouse: false,
				},
			],
		}
	}

	#[test]
	fn scene_sort_is_stable_by_z_then_id() {
		let mut scene = sample_scene();
		scene.sort_stable();
		let ids: Vec<u64> = scene.surfaces.iter().map(|s| s.id.0).collect();
		assert_eq!(ids, vec![1, 10, 2]);
	}

	#[test]
	fn hit_test_returns_topmost_mouse_surface() {
		let mut scene = sample_scene();
		scene.sort_stable();
		let hit = scene.hit_test(3, 3).expect("expected a hit");
		assert_eq!(hit.kind, SurfaceKind::Panels);
	}
}
