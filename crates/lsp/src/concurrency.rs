//! Incoming request multiplexing limits and cancellation.
//!
//! *Applies to both Language Servers and Language Clients.*
//!
//! Note that the [`crate::MainLoop`] can poll multiple ongoing requests
//! out-of-box, while this middleware is to provides these additional features:
//! 1. Limit concurrent incoming requests to at most `max_concurrency`.
//! 2. Cancellation of incoming requests via client notification `$/cancelRequest`.
use std::collections::HashMap;
use std::future::Future;
use std::num::NonZeroUsize;
use std::ops::ControlFlow;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::thread::available_parallelism;

use lsp_types::notification::{self, Notification};
use pin_project_lite::pin_project;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use tower_layer::Layer;
use tower_service::Service;

use crate::{
	AnyEvent, AnyNotification, AnyRequest, ErrorCode, LspService, RequestId, ResponseError, Result,
};

struct CancelState {
	notify: Notify,
	done: AtomicBool,
	cancelled: AtomicBool,
}

struct DoneSignaller(Arc<CancelState>);

impl Drop for DoneSignaller {
	fn drop(&mut self) {
		self.0.done.store(true, Ordering::Relaxed);
	}
}

pub(super) type PermitFuture =
	crate::router::BoxFutureStatic<Result<OwnedSemaphorePermit, tokio::sync::AcquireError>>;

/// The middleware for incoming request multiplexing limits and cancellation.
///
/// See [module level documentations](self) for details.
pub struct Concurrency<S> {
	/// The wrapped inner service.
	service: S,
	/// Maximum number of concurrent requests allowed.
	max_concurrency: NonZeroUsize,
	/// Semaphore for limiting concurrency.
	semaphore: Arc<Semaphore>,
	/// Pending permit acquisition.
	ready_fut: Option<PermitFuture>,
	/// Acquired permit for the next call.
	ready_permit: Option<OwnedSemaphorePermit>,
	/// Map of in-flight request IDs to their cancellation states.
	ongoing: HashMap<RequestId, Arc<CancelState>>,
}

define_getters!(impl[S] Concurrency<S>, service: S);

impl<S: LspService> Service<AnyRequest> for Concurrency<S>
where
	S::Error: From<ResponseError>,
{
	type Response = S::Response;
	type Error = S::Error;
	type Future = ResponseFuture<S::Future>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		if self.ready_permit.is_some() {
			return Poll::Ready(Ok(()));
		}

		if self.ready_fut.is_none() {
			let sema = self.semaphore.clone();
			self.ready_fut = Some(Box::pin(async move { sema.acquire_owned().await }));
		}

		let fut = self.ready_fut.as_mut().unwrap();
		match fut.as_mut().poll(cx) {
			Poll::Pending => Poll::Pending,
			Poll::Ready(Ok(permit)) => {
				self.ready_fut = None;
				self.ready_permit = Some(permit);
				Poll::Ready(Ok(()))
			}
			Poll::Ready(Err(_)) => {
				// Semaphore closed? Should not happen in normal lifecycle.
				Poll::Ready(Err(ResponseError::new(
					ErrorCode::INTERNAL_ERROR,
					"concurrency semaphore closed",
				)
				.into()))
			}
		}
	}

	fn call(&mut self, req: AnyRequest) -> Self::Future {
		let permit = self
			.ready_permit
			.take()
			.expect("poll_ready not called before call");

		// Purge completed tasks
		if self.ongoing.len() >= self.max_concurrency.get() * 2 {
			self.ongoing
				.retain(|_, st| !st.done.load(Ordering::Relaxed));
		}

		let st = Arc::new(CancelState {
			notify: Notify::new(),
			done: AtomicBool::new(false),
			cancelled: AtomicBool::new(false),
		});
		self.ongoing.insert(req.id.clone(), st.clone());

		let fut = self.service.call(req);
		ResponseFuture {
			fut,
			permit,
			st: st.clone(),
			_signaller: DoneSignaller(st),
		}
	}
}

pin_project! {
	/// The [`Future`] type used by the [`Concurrency`] middleware.
	pub struct ResponseFuture<Fut> {
		#[pin]
		fut: Fut,
		permit: OwnedSemaphorePermit,
		st: Arc<CancelState>,
		_signaller: DoneSignaller,
	}
}

impl<Fut, Response, Error> Future for ResponseFuture<Fut>
where
	Fut: Future<Output = Result<Response, Error>>,
	Error: From<ResponseError>,
{
	type Output = Fut::Output;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();

		// Fast path for cancellation
		if this.st.cancelled.load(Ordering::Relaxed) {
			return Poll::Ready(Err(ResponseError::new(
				ErrorCode::REQUEST_CANCELLED,
				"Client cancelled the request",
			)
			.into()));
		}

		// Poll the actual work
		if let Poll::Ready(res) = this.fut.poll(cx) {
			return Poll::Ready(res);
		}

		// Check for cancellation signal
		let mut n = this.st.notify.notified();
		// SAFETY: we only poll this locally and don't move it while polled.
		let mut n_pinned = unsafe { Pin::new_unchecked(&mut n) };
		if let Poll::Ready(()) = n_pinned.as_mut().poll(cx) {
			this.st.cancelled.store(true, Ordering::Relaxed);
			return Poll::Ready(Err(ResponseError::new(
				ErrorCode::REQUEST_CANCELLED,
				"Client cancelled the request",
			)
			.into()));
		}

		Poll::Pending
	}
}

// Remove the manual Drop implementation here as DoneSignaller handles it

impl<S: LspService> LspService for Concurrency<S>
where
	S::Error: From<ResponseError>,
{
	fn notify(&mut self, notif: AnyNotification) -> ControlFlow<Result<()>> {
		if notif.method == notification::Cancel::METHOD {
			if let Ok(params) = serde_json::from_value::<lsp_types::CancelParams>(notif.params)
				&& let Some(st) = self.ongoing.remove(&params.id)
			{
				st.cancelled.store(true, Ordering::Relaxed);
				st.notify.notify_waiters();
			}
			return ControlFlow::Continue(());
		}
		self.service.notify(notif)
	}

	fn emit(&mut self, event: AnyEvent) -> ControlFlow<Result<()>> {
		self.service.emit(event)
	}
}

/// The builder of [`Concurrency`] middleware.
///
/// It's [`Default`] configuration has `max_concurrency` of the result of
/// [`std::thread::available_parallelism`], fallback to `1` if it fails.
///
/// See [module level documentations](self) for details.
#[derive(Clone, Debug)]
#[must_use]
pub struct ConcurrencyBuilder {
	/// Maximum number of concurrent requests allowed.
	max_concurrency: NonZeroUsize,
}

impl Default for ConcurrencyBuilder {
	fn default() -> Self {
		Self::new(available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap()))
	}
}

impl ConcurrencyBuilder {
	/// Create the middleware with concurrency limit `max_concurrency`.
	pub fn new(max_concurrency: NonZeroUsize) -> Self {
		Self { max_concurrency }
	}
}

/// A type alias of [`ConcurrencyBuilder`] conforming to the naming convention of [`tower_layer`].
pub type ConcurrencyLayer = ConcurrencyBuilder;

impl<S> Layer<S> for ConcurrencyBuilder {
	type Service = Concurrency<S>;

	fn layer(&self, inner: S) -> Self::Service {
		Concurrency {
			service: inner,
			max_concurrency: self.max_concurrency,
			semaphore: Arc::new(Semaphore::new(self.max_concurrency.get())),
			ready_fut: None,
			ready_permit: None,
			ongoing: HashMap::with_capacity(self.max_concurrency.get() * 2),
		}
	}
}
