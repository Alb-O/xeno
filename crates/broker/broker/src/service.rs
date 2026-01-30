//! Broker service implementation.

use std::ops::ControlFlow;

use tower_service::Service;
use xeno_broker_proto::types::{ErrorCode, Event, Request, RequestPayload, ResponsePayload};
use xeno_rpc::{AnyEvent, RpcService};

use crate::protocol::BrokerProtocol;

/// Broker service state and request handlers.
#[derive(Debug, Default)]
pub struct BrokerService;

impl BrokerService {
	/// Create a new broker service instance.
	#[must_use]
	pub fn new() -> Self {
		Self
	}
}

impl Service<Request> for BrokerService {
	type Response = ResponsePayload;
	type Error = ErrorCode;
	type Future = std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
	>;

	fn poll_ready(
		&mut self,
		_cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Result<(), Self::Error>> {
		std::task::Poll::Ready(Ok(()))
	}

	fn call(&mut self, req: Request) -> Self::Future {
		let payload = req.payload;

		Box::pin(async move {
			let response = match payload {
				RequestPayload::Ping => ResponsePayload::Pong,
				RequestPayload::Subscribe { .. } => ResponsePayload::Subscribed,
				RequestPayload::LspStart { .. } => {
					ResponsePayload::Error(ErrorCode::NotImplemented)
				}
				RequestPayload::LspSend { .. } => ResponsePayload::Error(ErrorCode::NotImplemented),
			};

			Ok(response)
		})
	}
}

impl RpcService<BrokerProtocol> for BrokerService {
	type LoopError = std::io::Error;

	fn notify(&mut self, _notif: Event) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		ControlFlow::Continue(())
	}

	fn emit(&mut self, _event: AnyEvent) -> ControlFlow<std::result::Result<(), Self::LoopError>> {
		ControlFlow::Continue(())
	}
}
