use xeno_tui::layout::Rect;

use crate::ui::scene::{Surface, SurfaceId, SurfaceKind, SurfaceOp, UiScene, ZIndex};

pub struct SceneBuilder {
	next_id: u64,
	screen: Rect,
	main_area: Rect,
	doc_area: Rect,
	status_area: Rect,
	surfaces: Vec<Surface>,
}

impl SceneBuilder {
	pub fn new(screen: Rect, main_area: Rect, doc_area: Rect, status_area: Rect) -> Self {
		Self {
			next_id: 1,
			screen,
			main_area,
			doc_area,
			status_area,
			surfaces: Vec::new(),
		}
	}

	pub fn push(&mut self, kind: SurfaceKind, z: ZIndex, area: Rect, op: SurfaceOp, accepts_mouse: bool) -> SurfaceId {
		let id = SurfaceId(self.next_id);
		self.next_id += 1;
		self.surfaces.push(Surface {
			id,
			kind,
			z,
			area,
			op,
			accepts_mouse,
		});
		id
	}

	pub fn finish(self) -> UiScene {
		let mut scene = UiScene {
			screen: self.screen,
			main_area: self.main_area,
			doc_area: self.doc_area,
			status_area: self.status_area,
			surfaces: self.surfaces,
		};
		scene.sort_stable();
		scene
	}
}

pub trait UiLayer {
	fn name(&self) -> &'static str;
	fn build(&mut self, builder: &mut SceneBuilder);
}
