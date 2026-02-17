use rustc_hash::FxHashSet;
use smallvec::SmallVec;
use xeno_registry::actions::editor_ctx::OverlayRequest;
use xeno_registry::notifications::Notification;

use crate::overlay::LayerEvent;

#[derive(Debug, Default)]
pub struct EffectSink {
	/// If true, next frame MUST render.
	pub(crate) wants_redraw: bool,

	/// User-visible notifications to enqueue.
	pub(crate) notifications: SmallVec<[Notification; 4]>,

	/// Overlay/layer events to deliver (passive layers, etc.).
	pub(crate) layer_events: Vec<LayerEvent>,

	/// Overlay requests (modal open/close, info popup).
	pub(crate) overlay_requests: SmallVec<[OverlayRequest; 4]>,

	/// Invocations to queue for deferred runtime execution.
	pub(crate) queued_invocations: Vec<crate::types::Invocation>,
}

impl EffectSink {
	#[inline]
	pub fn request_redraw(&mut self) {
		self.wants_redraw = true;
	}

	#[inline]
	pub fn notify(&mut self, n: Notification) {
		self.notifications.push(n);
	}

	#[inline]
	pub fn push_layer_event(&mut self, e: LayerEvent) {
		self.layer_events.push(e);
	}

	#[inline]
	pub fn overlay_request(&mut self, r: OverlayRequest) {
		self.overlay_requests.push(r);
	}

	#[inline]
	pub fn defer_command(&mut self, name: String, args: Vec<String>) {
		self.queued_invocations.push(crate::types::Invocation::command(name, args));
	}

	#[inline]
	pub fn queue_invocation(&mut self, invocation: crate::types::Invocation) {
		self.queued_invocations.push(invocation);
	}

	pub fn drain(&mut self) -> DrainedEffects {
		let mut layer_events = std::mem::take(&mut self.layer_events);

		// Coalesce CursorMoved and ModeChanged events (keeping the LAST one)
		let mut cursor_moved = FxHashSet::default();
		let mut mode_changed = FxHashSet::default();

		layer_events.reverse();
		layer_events.retain(|e| match e {
			LayerEvent::CursorMoved { view } => cursor_moved.insert(*view),
			LayerEvent::ModeChanged { view, .. } => mode_changed.insert(*view),
			_ => true,
		});
		layer_events.reverse();

		DrainedEffects {
			wants_redraw: std::mem::take(&mut self.wants_redraw),
			notifications: self.notifications.drain(..).collect(),
			layer_events,
			overlay_requests: self.overlay_requests.drain(..).collect(),
			queued_invocations: std::mem::take(&mut self.queued_invocations),
		}
	}
}

pub struct DrainedEffects {
	pub wants_redraw: bool,
	pub notifications: Vec<Notification>,
	pub layer_events: Vec<LayerEvent>,
	pub overlay_requests: Vec<OverlayRequest>,
	pub queued_invocations: Vec<crate::types::Invocation>,
}

impl DrainedEffects {
	pub fn is_empty(&self) -> bool {
		!self.wants_redraw
			&& self.notifications.is_empty()
			&& self.layer_events.is_empty()
			&& self.overlay_requests.is_empty()
			&& self.queued_invocations.is_empty()
	}
}
