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
	SnippetChoicePopup,
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
	SnippetChoicePopup,
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
#[allow(dead_code)]
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

	#[allow(dead_code)]
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
