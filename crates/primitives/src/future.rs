use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// A pinned, boxed future that is not required to be Send.
pub type BoxFutureLocal<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// A pinned, boxed future that is required to be Send.
pub type BoxFutureSend<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A pinned, boxed future that is required to be Send and 'static.
pub type BoxFutureStatic<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Polls a future once without registering for wakeups.
///
/// Only use this if you know the future is ready or
/// if you are intentionally performing a non-blocking check.
pub fn poll_once<F: Future + Unpin>(mut fut: F) -> Option<F::Output> {
	let noop_waker = unsafe { Waker::from_raw(noop_raw_waker()) };
	let mut cx = Context::from_waker(&noop_waker);
	match Pin::new(&mut fut).poll(&mut cx) {
		Poll::Ready(res) => Some(res),
		Poll::Pending => None,
	}
}

fn noop_raw_waker() -> RawWaker {
	fn noop(_: *const ()) {}
	fn clone(_: *const ()) -> RawWaker {
		noop_raw_waker()
	}
	let vtable = &RawWakerVTable::new(clone, noop, noop, noop);
	RawWaker::new(std::ptr::null(), vtable)
}
