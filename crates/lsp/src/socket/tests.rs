use super::*;

#[tokio::test]
async fn closed_client_socket() {
	let socket = ClientSocket::new_closed();
	assert!(matches!(
		socket.notify::<lsp_types::notification::Exit>(()),
		Err(Error::ServiceStopped)
	));
	assert!(matches!(
		socket.request::<lsp_types::request::Shutdown>(()).await,
		Err(Error::ServiceStopped)
	));
	assert!(matches!(socket.emit(42i32), Err(Error::ServiceStopped)));
}

#[tokio::test]
async fn closed_server_socket() {
	let socket = ServerSocket::new_closed();
	assert!(matches!(
		socket.notify::<lsp_types::notification::Exit>(()),
		Err(Error::ServiceStopped)
	));
	assert!(matches!(
		socket.request::<lsp_types::request::Shutdown>(()).await,
		Err(Error::ServiceStopped)
	));
	assert!(matches!(socket.emit(42i32), Err(Error::ServiceStopped)));
}
