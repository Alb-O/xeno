//! Local OAuth callback server for Codex authentication.
//!
//! Handles the OAuth redirect by spinning up a temporary HTTP server
//! on localhost to receive the authorization code.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::{io, thread};

use tiny_http::{Header, Request, Response, Server};
use tokio::sync::{Notify, mpsc};

use super::client::{ExchangedTokens, exchange_code_for_tokens};
use super::constants::{CLIENT_ID, DEFAULT_PORT, ISSUER, ORIGINATOR, SCOPE};
use super::storage::save_auth;
use super::token::{AuthState, TokenData, jwt_auth_claims, parse_id_token};
use crate::error::{AuthError, AuthResult};
use crate::pkce::{PkceCodes, generate_state};

/// Configuration for the login server.
#[derive(Debug, Clone)]
pub struct LoginConfig {
	/// XDG data directory for storing auth state.
	pub data_dir: PathBuf,

	/// OAuth client ID.
	pub client_id: String,

	/// OAuth issuer URL.
	pub issuer: String,

	/// Local port for callback server.
	pub port: u16,

	/// Whether to open browser automatically.
	pub open_browser: bool,

	/// Force a specific state value (for testing).
	pub force_state: Option<String>,

	/// Required workspace ID (if set, only this workspace can login).
	pub required_workspace_id: Option<String>,
}

impl Default for LoginConfig {
	fn default() -> Self {
		Self {
			data_dir: PathBuf::new(),
			client_id: CLIENT_ID.to_string(),
			issuer: ISSUER.to_string(),
			port: DEFAULT_PORT,
			open_browser: true,
			force_state: None,
			required_workspace_id: None,
		}
	}
}

impl LoginConfig {
	/// Create a new config with the given data directory.
	pub fn new(data_dir: PathBuf) -> Self {
		Self {
			data_dir,
			..Default::default()
		}
	}
}

/// Handle for cancelling a running login server.
#[derive(Clone, Debug)]
pub struct ShutdownHandle {
	notify: Arc<Notify>,
}

impl ShutdownHandle {
	/// Signal the server to shut down.
	pub fn shutdown(&self) {
		self.notify.notify_waiters();
	}
}

/// Running login server instance.
pub struct LoginServer {
	/// URL to open in browser for authentication.
	pub auth_url: String,

	/// Actual port the server bound to.
	pub port: u16,

	/// Handle to the server task.
	handle: tokio::task::JoinHandle<AuthResult<()>>,

	/// Shutdown signal.
	shutdown: ShutdownHandle,
}

impl LoginServer {
	/// Wait for the login to complete.
	pub async fn wait(self) -> AuthResult<()> {
		self.handle
			.await
			.map_err(|e| AuthError::Io(io::Error::other(format!("task panicked: {e}"))))?
	}

	/// Get a handle to cancel the login.
	pub fn shutdown_handle(&self) -> ShutdownHandle {
		self.shutdown.clone()
	}

	/// Cancel the login.
	pub fn cancel(&self) {
		self.shutdown.shutdown();
	}
}

/// Start the OAuth login flow.
///
/// This will:
/// 1. Generate PKCE codes and state
/// 2. Start a local HTTP server for the callback
/// 3. Open the browser to the authorization URL
/// 4. Wait for the callback and exchange the code for tokens
/// 5. Save the tokens to disk
pub fn start_login(config: LoginConfig) -> AuthResult<LoginServer> {
	let pkce = PkceCodes::generate();
	let state = config.force_state.clone().unwrap_or_else(generate_state);

	let server = bind_server(config.port)?;
	let port = match server.server_addr().to_ip() {
		Some(addr) => addr.port(),
		None => {
			return Err(AuthError::PortBinding {
				port: config.port,
				reason: "unable to determine bound port".into(),
			});
		}
	};
	let server = Arc::new(server);

	let redirect_uri = format!("http://localhost:{port}/auth/callback");
	let auth_url = build_authorize_url(
		&config.issuer,
		&config.client_id,
		&redirect_uri,
		&pkce,
		&state,
		config.required_workspace_id.as_deref(),
	);

	if config.open_browser {
		let _ = webbrowser::open(&auth_url);
	}

	let (tx, mut rx) = mpsc::channel::<Request>(16);
	let server_clone = server.clone();
	thread::spawn(move || {
		while let Ok(request) = server_clone.recv() {
			if tx.blocking_send(request).is_err() {
				break;
			}
		}
	});

	let shutdown_notify = Arc::new(Notify::new());
	let shutdown = ShutdownHandle {
		notify: shutdown_notify.clone(),
	};

	let handle = tokio::spawn(async move {
		let result = loop {
			tokio::select! {
				_ = shutdown_notify.notified() => {
					break Err(AuthError::Cancelled);
				}
				maybe_req = rx.recv() => {
					let Some(req) = maybe_req else {
						break Err(AuthError::Cancelled);
					};

					let url = req.url().to_string();
					match process_request(
						&url,
						&config,
						&redirect_uri,
						&pkce,
						port,
						&state,
					).await {
						RequestResult::Continue(response) => {
							let _ = tokio::task::spawn_blocking(move || {
								let _ = req.respond(response);
							}).await;
						}
						RequestResult::Redirect(location) => {
							if let Ok(header) = Header::from_bytes(
								&b"Location"[..],
								location.as_bytes(),
							) {
								let response = Response::empty(302).with_header(header);
								let _ = tokio::task::spawn_blocking(move || {
									let _ = req.respond(response);
								}).await;
							}
						}
						RequestResult::Done { body, result } => {
							let _ = tokio::task::spawn_blocking(move || {
								send_final_response(req, body);
							}).await;
							break result;
						}
					}
				}
			}
		};

		server.unblock();
		result
	});

	Ok(LoginServer {
		auth_url,
		port,
		handle,
		shutdown,
	})
}

enum RequestResult {
	Continue(Response<std::io::Cursor<Vec<u8>>>),
	Redirect(String),
	Done {
		body: Vec<u8>,
		result: AuthResult<()>,
	},
}

async fn process_request(
	url: &str,
	config: &LoginConfig,
	redirect_uri: &str,
	pkce: &PkceCodes,
	port: u16,
	state: &str,
) -> RequestResult {
	let parsed = match url::Url::parse(&format!("http://localhost{url}")) {
		Ok(u) => u,
		Err(_) => {
			return RequestResult::Continue(
				Response::from_string("Bad Request").with_status_code(400),
			);
		}
	};

	match parsed.path() {
		"/auth/callback" => {
			let params: std::collections::HashMap<String, String> =
				parsed.query_pairs().into_owned().collect();

			if params.get("state").map(String::as_str) != Some(state) {
				return RequestResult::Done {
					body: b"State mismatch".to_vec(),
					result: Err(AuthError::StateMismatch),
				};
			}

			let Some(code) = params.get("code").filter(|c| !c.is_empty()) else {
				return RequestResult::Done {
					body: b"Missing authorization code".to_vec(),
					result: Err(AuthError::MissingCode),
				};
			};

			let tokens = match exchange_code_for_tokens(
				&config.issuer,
				&config.client_id,
				redirect_uri,
				pkce,
				code,
			)
			.await
			{
				Ok(t) => t,
				Err(e) => {
					return RequestResult::Done {
						body: format!("Token exchange failed: {e}").into_bytes(),
						result: Err(e),
					};
				}
			};

			if let Some(ref required) = config.required_workspace_id {
				let claims = jwt_auth_claims(&tokens.id_token);
				let actual = claims.get("chatgpt_account_id").and_then(|v| v.as_str());

				if actual != Some(required.as_str()) {
					let msg = format!("Login restricted to workspace {required}");
					return RequestResult::Done {
						body: msg.as_bytes().to_vec(),
						result: Err(AuthError::WorkspaceRestriction(msg)),
					};
				}
			}

			if let Err(e) = save_tokens(&config.data_dir, &tokens).await {
				return RequestResult::Done {
					body: format!("Failed to save auth: {e}").into_bytes(),
					result: Err(e),
				};
			}

			let success_url = format!("http://localhost:{port}/success");
			RequestResult::Redirect(success_url)
		}

		"/success" => {
			let body = include_str!("assets/success.html");
			RequestResult::Done {
				body: body.as_bytes().to_vec(),
				result: Ok(()),
			}
		}

		"/cancel" => RequestResult::Done {
			body: b"Login cancelled".to_vec(),
			result: Err(AuthError::Cancelled),
		},

		_ => RequestResult::Continue(Response::from_string("Not Found").with_status_code(404)),
	}
}

fn send_final_response(req: Request, body: Vec<u8>) {
	let mut writer = req.into_writer();
	let _ = write!(writer, "HTTP/1.1 200 OK\r\n");
	let _ = write!(writer, "Content-Type: text/html; charset=utf-8\r\n");
	let _ = write!(writer, "Content-Length: {}\r\n", body.len());
	let _ = write!(writer, "Connection: close\r\n\r\n");
	let _ = writer.write_all(&body);
	let _ = writer.flush();
}

async fn save_tokens(data_dir: &Path, tokens: &ExchangedTokens) -> AuthResult<()> {
	let data_dir = data_dir.to_path_buf();
	let id_token = tokens.id_token.clone();
	let access_token = tokens.access_token.clone();
	let refresh_token = tokens.refresh_token.clone();

	tokio::task::spawn_blocking(move || {
		let parsed = parse_id_token(&id_token)?;
		let account_id = parsed.account_id.clone();

		let token_data = TokenData {
			id_token: parsed,
			access_token,
			refresh_token,
			account_id,
		};

		let state = AuthState::from_tokens(token_data);
		save_auth(&data_dir, &state)
	})
	.await
	.map_err(|e| AuthError::Storage(format!("task failed: {e}")))?
}

fn build_authorize_url(
	issuer: &str,
	client_id: &str,
	redirect_uri: &str,
	pkce: &PkceCodes,
	state: &str,
	required_workspace_id: Option<&str>,
) -> String {
	let mut params = vec![
		("response_type", "code".to_string()),
		("client_id", client_id.to_string()),
		("redirect_uri", redirect_uri.to_string()),
		("scope", SCOPE.to_string()),
		("code_challenge", pkce.challenge.clone()),
		("code_challenge_method", "S256".to_string()),
		("state", state.to_string()),
		("id_token_add_organizations", "true".to_string()),
		("codex_cli_simplified_flow", "true".to_string()),
		("originator", ORIGINATOR.to_string()),
	];

	if let Some(workspace_id) = required_workspace_id {
		params.push(("allowed_workspace_id", workspace_id.to_string()));
	}

	let query = params
		.into_iter()
		.map(|(k, v)| format!("{}={}", k, urlencoding::encode(&v)))
		.collect::<Vec<_>>()
		.join("&");

	format!("{issuer}/oauth/authorize?{query}")
}

fn bind_server(port: u16) -> AuthResult<Server> {
	let addr = format!("127.0.0.1:{port}");
	let mut attempts = 0;
	const MAX_ATTEMPTS: u32 = 10;
	const RETRY_DELAY: Duration = Duration::from_millis(200);
	let mut cancel_attempted = false;

	loop {
		match Server::http(&addr) {
			Ok(server) => return Ok(server),
			Err(e) => {
				attempts += 1;

				let is_addr_in_use = e
					.downcast_ref::<io::Error>()
					.map(|e| e.kind() == io::ErrorKind::AddrInUse)
					.unwrap_or(false);

				if is_addr_in_use {
					if !cancel_attempted {
						cancel_attempted = true;
						let _ = send_cancel_request(port);
					}

					thread::sleep(RETRY_DELAY);

					if attempts >= MAX_ATTEMPTS {
						return Err(AuthError::PortBinding {
							port,
							reason: "port already in use".into(),
						});
					}

					continue;
				}

				return Err(AuthError::PortBinding {
					port,
					reason: e.to_string(),
				});
			}
		}
	}
}

fn send_cancel_request(port: u16) -> io::Result<()> {
	let addr: SocketAddr = format!("127.0.0.1:{port}")
		.parse()
		.map_err(io::Error::other)?;
	let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
	stream.set_read_timeout(Some(Duration::from_secs(2)))?;
	stream.set_write_timeout(Some(Duration::from_secs(2)))?;

	stream.write_all(b"GET /cancel HTTP/1.1\r\n")?;
	stream.write_all(format!("Host: 127.0.0.1:{port}\r\n").as_bytes())?;
	stream.write_all(b"Connection: close\r\n\r\n")?;

	let mut buf = [0u8; 64];
	let _ = stream.read(&mut buf);
	Ok(())
}
