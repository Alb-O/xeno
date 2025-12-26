//! Integration tests for ACP with the big-pickle model.
//!
//! These tests require the PICKLE_TESTS=1 environment variable to run.
//! They make actual calls to the big-pickle AI model via OpenCode's ACP server.
//!
//! Run with: PICKLE_TESTS=1 cargo test -p tome-acp --test pickle_integration -- --nocapture

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::{
	Agent, Client, ClientCapabilities, ClientSideConnection, ContentBlock, FileSystemCapability,
	Implementation, InitializeRequest, NewSessionRequest, PromptRequest, ProtocolVersion,
	RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
	SelectedPermissionOutcome, SessionNotification, SessionUpdate, TextContent,
};
use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::task::LocalSet;
use tokio::time::timeout;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

/// Check if pickle tests should run.
fn require_pickle() -> bool {
	if std::env::var("PICKLE_TESTS").is_ok() {
		true
	} else {
		eprintln!("Skipping pickle test (set PICKLE_TESTS=1 to run)");
		false
	}
}

/// Get a temporary directory for tests.
fn test_cwd() -> PathBuf {
	std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Simple client handler that collects responses.
#[derive(Clone)]
struct TestClient {
	messages: Arc<Mutex<Vec<String>>>,
}

#[async_trait(?Send)]
impl Client for TestClient {
	async fn request_permission(
		&self,
		req: RequestPermissionRequest,
	) -> agent_client_protocol::Result<RequestPermissionResponse> {
		// Auto-approve all permission requests
		if !req.options.is_empty() {
			let outcome = RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
				req.options[0].option_id.clone(),
			));
			Ok(RequestPermissionResponse::new(outcome))
		} else {
			Err(agent_client_protocol::Error::new(
				-32000,
				"No permission options",
			))
		}
	}

	async fn session_notification(
		&self,
		notification: SessionNotification,
	) -> agent_client_protocol::Result<()> {
		match notification.update {
			SessionUpdate::AgentMessageChunk(chunk) => {
				if let ContentBlock::Text(text) = chunk.content {
					let mut msgs = self.messages.lock();
					msgs.push(text.text);
				}
			}
			SessionUpdate::AgentThoughtChunk(chunk) => {
				if let ContentBlock::Text(text) = chunk.content {
					eprintln!("[thought] {}", text.text);
				}
			}
			_ => {}
		}
		Ok(())
	}
}

/// Spawn the opencode ACP server and return a connection.
async fn spawn_acp_connection(
	cwd: PathBuf,
) -> anyhow::Result<(Arc<ClientSideConnection>, String, Arc<Mutex<Vec<String>>>)> {
	let mut child = Command::new("opencode")
		.arg("acp")
		.arg("--port")
		.arg("0")
		.current_dir(&cwd)
		.stdin(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.spawn()?;

	let stdin = child.stdin.take().unwrap();
	let stdout = child.stdout.take().unwrap();
	let stderr = child.stderr.take();

	// Spawn stderr reader for debugging
	if let Some(stderr) = stderr {
		tokio::task::spawn_local(async move {
			let mut lines = BufReader::new(stderr).lines();
			while let Ok(Some(line)) = lines.next_line().await {
				eprintln!("[acp stderr] {}", line);
			}
		});
	}

	let messages = Arc::new(Mutex::new(Vec::new()));
	let handler = TestClient {
		messages: messages.clone(),
	};

	let (conn, io_fut) =
		ClientSideConnection::new(handler, stdin.compat_write(), stdout.compat(), |fut| {
			tokio::task::spawn_local(fut);
		});

	// Spawn IO task
	tokio::task::spawn_local(async move {
		if let Err(e) = io_fut.await {
			eprintln!("ACP IO error: {:?}", e);
		}
	});

	let conn = Arc::new(conn);

	// Initialize
	let init_res = conn
		.initialize(
			InitializeRequest::new(ProtocolVersion::from(1))
				.client_capabilities(
					ClientCapabilities::new()
						.fs(FileSystemCapability::new()
							.read_text_file(false)
							.write_text_file(false))
						.terminal(false),
				)
				.client_info(Implementation::new("TomeTest", "0.1.0").title("Tome Test")),
		)
		.await?;

	eprintln!(
		"Connected to agent: {:?} (v{})",
		init_res.agent_info, init_res.protocol_version
	);

	// Create session
	let session_res = conn.new_session(NewSessionRequest::new(cwd)).await?;
	let session_id = session_res.session_id.to_string();

	eprintln!("Session created: {}", session_id);

	// Log model info if available
	if let Some(models) = session_res.models {
		eprintln!("Current model: {}", models.current_model_id);
		eprintln!(
			"Available models: {:?}",
			models
				.available_models
				.iter()
				.map(|m| m.model_id.to_string())
				.collect::<Vec<_>>()
		);
	}

	Ok((conn, session_id, messages))
}

/// Wait for messages to accumulate and return them.
async fn wait_for_response(
	messages: &Arc<Mutex<Vec<String>>>,
	timeout_duration: Duration,
) -> Vec<String> {
	let start = std::time::Instant::now();

	// Wait a bit for the response to come in
	loop {
		tokio::time::sleep(Duration::from_millis(100)).await;

		let msgs = messages.lock();
		if !msgs.is_empty() {
			// Wait a bit more for the full response
			drop(msgs);
			tokio::time::sleep(Duration::from_millis(500)).await;
			break;
		}

		if start.elapsed() > timeout_duration {
			break;
		}
	}

	let msgs = messages.lock();
	msgs.clone()
}

/// Run a test with LocalSet for spawn_local support.
fn run_local_test<F>(test_fn: F)
where
	F: std::future::Future<Output = ()>,
{
	let rt = tokio::runtime::Runtime::new().unwrap();
	let local = LocalSet::new();
	local.block_on(&rt, test_fn);
}

#[test]
fn test_acp_connection_and_session() {
	if !require_pickle() {
		return;
	}

	let cwd = test_cwd();

	run_local_test(async move {
		let result = timeout(Duration::from_secs(30), spawn_acp_connection(cwd)).await;

		match result {
			Ok(Ok((conn, session_id, _messages))) => {
				assert!(!session_id.is_empty(), "Session ID should not be empty");
				eprintln!("Successfully connected and created session: {}", session_id);

				// Test that connection is alive by checking we can still use it
				drop(conn);
			}
			Ok(Err(e)) => {
				panic!("Failed to connect to ACP: {:?}", e);
			}
			Err(_) => {
				panic!("Timeout waiting for ACP connection");
			}
		}
	});
}

#[test]
fn test_acp_simple_prompt() {
	if !require_pickle() {
		return;
	}

	let cwd = test_cwd();

	run_local_test(async move {
		let result = timeout(Duration::from_secs(60), async {
			let (conn, session_id, messages) = spawn_acp_connection(cwd).await?;

			// Send a simple prompt
			let prompt = "Say 'Hello from pickle test' and nothing else.";
			eprintln!("Sending prompt: {}", prompt);

			let req = PromptRequest::new(
				session_id.clone(),
				vec![ContentBlock::Text(TextContent::new(prompt))],
			);

			// Send prompt (this returns when processing is complete)
			let _response = conn.prompt(req).await?;

			// Wait for and collect response messages
			let response_msgs = wait_for_response(&messages, Duration::from_secs(30)).await;

			Ok::<_, anyhow::Error>(response_msgs)
		})
		.await;

		match result {
			Ok(Ok(response_msgs)) => {
				let full_response = response_msgs.join("");
				eprintln!("Got response: {}", full_response);

				assert!(
					!full_response.is_empty(),
					"Should receive a non-empty response from the model"
				);

				// The model should mention "hello" or "pickle" in some form
				let lower = full_response.to_lowercase();
				assert!(
					lower.contains("hello") || lower.contains("pickle"),
					"Response should contain 'hello' or 'pickle': {}",
					full_response
				);
			}
			Ok(Err(e)) => {
				panic!("Prompt failed: {:?}", e);
			}
			Err(_) => {
				panic!("Timeout waiting for prompt response");
			}
		}
	});
}

#[test]
fn test_acp_multi_turn_conversation() {
	if !require_pickle() {
		return;
	}

	let cwd = test_cwd();

	run_local_test(async move {
		let result = timeout(Duration::from_secs(120), async {
			let (conn, session_id, messages) = spawn_acp_connection(cwd).await?;

			// First turn: introduce a topic
			let prompt1 = "Remember the number 42. Just say 'OK, I will remember 42.'";
			eprintln!("Sending first prompt: {}", prompt1);

			let req1 = PromptRequest::new(
				session_id.clone(),
				vec![ContentBlock::Text(TextContent::new(prompt1))],
			);
			let _response1 = conn.prompt(req1).await?;

			let response1_msgs = wait_for_response(&messages, Duration::from_secs(30)).await;
			let response1 = response1_msgs.join("");
			eprintln!("First response: {}", response1);

			// Clear messages for second turn
			{
				let mut msgs = messages.lock();
				msgs.clear();
			}

			// Second turn: ask about the remembered topic
			let prompt2 = "What number did I ask you to remember? Just say the number.";
			eprintln!("Sending second prompt: {}", prompt2);

			let req2 = PromptRequest::new(
				session_id.clone(),
				vec![ContentBlock::Text(TextContent::new(prompt2))],
			);
			let _response2 = conn.prompt(req2).await?;

			let response2_msgs = wait_for_response(&messages, Duration::from_secs(30)).await;
			let response2 = response2_msgs.join("");
			eprintln!("Second response: {}", response2);

			Ok::<_, anyhow::Error>((response1, response2))
		})
		.await;

		match result {
			Ok(Ok((response1, response2))) => {
				assert!(!response1.is_empty(), "First response should not be empty");
				assert!(
					!response2.is_empty(),
					"Second response should not be empty"
				);

				// The second response should mention 42, showing context is maintained
				assert!(
					response2.contains("42"),
					"Second response should mention the remembered number 42: {}",
					response2
				);
			}
			Ok(Err(e)) => {
				panic!("Multi-turn conversation failed: {:?}", e);
			}
			Err(_) => {
				panic!("Timeout during multi-turn conversation");
			}
		}
	});
}
