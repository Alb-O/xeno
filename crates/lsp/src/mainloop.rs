//! Service main loop driver for Language Servers and Clients.

use std::future::Future;
use std::ops::ControlFlow;
use std::pin::Pin;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use serde_json::Value as JsonValue;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, BufReader};
use tower_service::Service;

use crate::event::AnyEvent;
use crate::protocol::JsonRpcProtocol;
use crate::socket::{ClientSocket, PeerSocket, ServerSocket};
use crate::types::{AnyRequest, ResponseError};
use crate::{Error, LspService, Result};

/// Macro to define getter methods for accessing inner service fields.
#[macro_export]
macro_rules! define_getters {
    (impl[$($generic:tt)*] $ty:ty, $field:ident : $field_ty:ty) => {
        impl<$($generic)*> $ty {
            /// Get a reference to the inner service.
            #[must_use]
            pub fn get_ref(&self) -> &$field_ty {
                &self.$field
            }

            /// Get a mutable reference to the inner service.
            #[must_use]
            pub fn get_mut(&mut self) -> &mut $field_ty {
                &mut self.$field
            }

            /// Consume self, returning the inner service.
            #[must_use]
            pub fn into_inner(self) -> $field_ty {
                self.$field
            }
        }
    };
}

/// Service main loop driver for either Language Servers or Language Clients.
pub struct MainLoop<S: LspService> {
	/// The underlying RPC main loop.
	inner: xeno_rpc::MainLoop<LspServiceWrapper<S>, JsonRpcProtocol>,
}

/// Wrapper that adapts LspService to RpcService.
pub(crate) struct LspServiceWrapper<S> {
	service: S,
}

impl<S> LspServiceWrapper<S> {
	pub(crate) fn new(service: S) -> Self {
		Self { service }
	}
}

/// Future that wraps the service future and maps the error type to ResponseError.
pin_project! {
	pub struct ServiceFuture<Fut> {
		#[pin]
		inner: Fut,
	}
}

impl<Fut> ServiceFuture<Fut> {
	fn new(inner: Fut) -> Self {
		Self { inner }
	}
}

impl<Fut, E> Future for ServiceFuture<Fut>
where
	Fut: Future<Output = std::result::Result<JsonValue, E>>,
	ResponseError: From<E>,
{
	type Output = std::result::Result<JsonValue, ResponseError>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();
		match this.inner.poll(cx) {
			Poll::Ready(Ok(v)) => Poll::Ready(Ok(v)),
			Poll::Ready(Err(e)) => Poll::Ready(Err(e.into())),
			Poll::Pending => Poll::Pending,
		}
	}
}

impl<S> Service<AnyRequest> for LspServiceWrapper<S>
where
	S: LspService<Response = JsonValue> + Send,
	S::Future: Send + 'static,
	ResponseError: From<S::Error>,
{
	type Response = JsonValue;
	type Error = ResponseError;
	type Future = ServiceFuture<S::Future>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
		self.service.poll_ready(cx).map_err(|e| e.into())
	}

	fn call(&mut self, req: AnyRequest) -> Self::Future {
		ServiceFuture::new(self.service.call(req))
	}
}

impl<S> xeno_rpc::RpcService<JsonRpcProtocol> for LspServiceWrapper<S>
where
	S: LspService<Response = JsonValue> + Send,
	S::Future: Send + 'static,
	ResponseError: From<S::Error>,
{
	type LoopError = Error;

	fn notify(
		&mut self,
		notif: crate::types::AnyNotification,
	) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		self.service.notify(notif)
	}

	fn emit(&mut self, event: AnyEvent) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		self.service.emit(event)
	}
}

define_getters!(impl[S: LspService<Response = JsonValue>] MainLoop<S>, inner: xeno_rpc::MainLoop<LspServiceWrapper<S>, JsonRpcProtocol>);

impl<S> MainLoop<S>
where
	S: LspService<Response = JsonValue> + Send + 'static,
	S::Future: Send + 'static,
	ResponseError: From<S::Error>,
{
	/// Create a Language Server main loop.
	#[must_use]
	pub fn new_server(builder: impl FnOnce(ClientSocket) -> S) -> (Self, ClientSocket) {
		let id_gen: i32 = 0;
		let protocol = JsonRpcProtocol::new();
		let (inner, rpc_socket) = xeno_rpc::MainLoop::new(
			|socket| LspServiceWrapper::new(builder(ClientSocket(PeerSocket::from_rpc(socket)))),
			protocol,
			id_gen,
		);
		let socket = ClientSocket(PeerSocket::from_rpc(rpc_socket));
		(Self { inner }, socket)
	}

	/// Create a Language Client main loop.
	#[must_use]
	pub fn new_client(builder: impl FnOnce(ServerSocket) -> S) -> (Self, ServerSocket) {
		let id_gen: i32 = 0;
		let protocol = JsonRpcProtocol::new();
		let (inner, rpc_socket) = xeno_rpc::MainLoop::new(
			|socket| LspServiceWrapper::new(builder(ServerSocket(PeerSocket::from_rpc(socket)))),
			protocol,
			id_gen,
		);
		let socket = ServerSocket(PeerSocket::from_rpc(rpc_socket));
		(Self { inner }, socket)
	}

	/// Drive the service main loop to provide the service.
	///
	/// Shortcut to [`MainLoop::run`] that accept an `impl AsyncRead` and implicit wrap it in a
	/// [`BufReader`].
	#[allow(clippy::missing_errors_doc, reason = "errors documented in Self::run")]
	pub async fn run_buffered(
		self,
		input: impl AsyncRead + Unpin + Send,
		output: impl AsyncWrite + Unpin + Send,
	) -> Result<()> {
		self.run(BufReader::new(input), output).await
	}

	/// Drive the service main loop to provide the service.
	///
	/// # Errors
	///
	/// - `Error::Io` when the underlying `input` or `output` raises an error.
	/// - `Error::Deserialize` when the peer sends undecodable or invalid message.
	/// - `Error::Protocol` when the peer violates Language Server Protocol.
	/// - Other errors raised from service handlers.
	pub async fn run(
		self,
		input: impl AsyncBufRead + Unpin + Send,
		output: impl AsyncWrite + Unpin + Send,
	) -> Result<()> {
		self.inner.run(input, output).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Error;

	fn _main_loop_future_is_send<S>(
		f: MainLoop<S>,
		input: impl AsyncBufRead + Send + Unpin,
		output: impl AsyncWrite + Send + Unpin,
	) -> impl Send
	where
		S: LspService<Response = JsonValue> + Send + 'static,
		S::Future: Send + 'static,
		ResponseError: From<S::Error>,
	{
		f.run(input, output)
	}
}
