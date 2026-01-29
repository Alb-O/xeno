use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// A pinned, boxed future that is not required to be Send.
pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// A pinned, boxed future that is required to be Send.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A pinned, boxed future that is required to be Send and 'static.
pub type BoxFutureStatic<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Simple helper to poll a future once and return the result if ready.
pub fn now_or_never<F: Future>(mut fut: F) -> Option<F::Output> {
	let fut = unsafe { Pin::new_unchecked(&mut fut) };
	let noop_waker = unsafe { Waker::from_raw(noop_raw_waker()) };
	let mut cx = Context::from_waker(&noop_waker);
	match fut.poll(&mut cx) {
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
