use tracing::warn;
use xeno_primitives::{Mode, Selection};
use xeno_registry::{MotionDispatchAccess, MotionKind, MotionRequest, motions};

use crate::capabilities::provider::EditorCaps;

impl MotionDispatchAccess for EditorCaps<'_> {
	fn apply_motion(&mut self, req: &MotionRequest) -> Selection {
		let Some(motion_key) = motions::find(req.id.as_str()) else {
			warn!("unknown motion: {}", req.id.as_str());
			return xeno_registry::SelectionAccess::selection(self).clone();
		};

		let handler = motion_key.def().handler;
		let selection = xeno_registry::SelectionAccess::selection(self).clone();
		let is_normal = xeno_registry::ModeAccess::mode(self) == Mode::Normal;

		let MotionRequest {
			count,
			extend,
			kind,
			..
		} = *req;

		let new_ranges = self.ed.buffer().with_doc(|doc| {
			let text = doc.content().slice(..);
			selection
				.ranges()
				.iter()
				.map(|range| {
					let mut target = handler(text, *range, count, extend);

					if is_normal {
						target.head = xeno_primitives::rope::clamp_to_cell(target.head, text);
					}

					match kind {
						MotionKind::Cursor if extend => {
							xeno_primitives::Range::new(range.anchor, target.head)
						}
						MotionKind::Cursor => xeno_primitives::Range::point(target.head),
						MotionKind::Selection => {
							xeno_primitives::Range::new(range.anchor, target.head)
						}
						MotionKind::Word if extend => {
							xeno_primitives::Range::new(range.anchor, target.head)
						}
						MotionKind::Word => target,
					}
				})
				.collect::<Vec<_>>()
		});

		Selection::from_vec(new_ranges, selection.primary_index())
	}
}
