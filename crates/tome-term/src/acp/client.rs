use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use agent_client_protocol::{
    Agent, AgentNotification, AgentRequest, CancelNotification, ClientCapabilities, ClientResponse,
    ClientSide, ClientSideConnection, ContentBlock, FileSystemCapability, Implementation,
    InitializeRequest, MessageHandler, NewSessionRequest, PromptRequest, ProtocolVersion,
    ReadTextFileResponse, RequestPermissionOutcome, RequestPermissionResponse, Result,
    SelectedPermissionOutcome, SessionUpdate, TextContent, WriteTextFileResponse,
};
use tokio::process::Command;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

pub enum AgentUiEvent {
    Connected {
        agent_name: String,
        protocol_version: String,
    },
    Disconnected {
        reason: String,
    },
    SessionStarted {
        session_id: String,
    },
    SessionUpdate(SessionUpdate),
    PermissionRequested {
        request_id: String,
    },
    ToolingError {
        message: String,
    },
}

pub enum AgentCommand {
    Start { cwd: PathBuf },
    Stop,
    Prompt { content: String },
    Cancel,
}

pub struct AcpClientRuntime {
    cmd_tx: Sender<AgentCommand>,
}

impl AcpClientRuntime {
    pub fn new(ui_tx: Sender<AgentUiEvent>) -> Self {
        let (cmd_tx, cmd_rx) = channel();

        thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            let local = LocalSet::new();

            local.block_on(&rt, async {
                AcpBackend::new(ui_tx, cmd_rx).run().await;
            });
        });

        Self { cmd_tx }
    }

    pub fn send(&self, cmd: AgentCommand) {
        let _ = self.cmd_tx.send(cmd);
    }
}

struct AcpBackend {
    ui_tx: Sender<AgentUiEvent>,
    cmd_rx: Receiver<AgentCommand>,
    session_id: Option<String>,
}

impl AcpBackend {
    fn new(ui_tx: Sender<AgentUiEvent>, cmd_rx: Receiver<AgentCommand>) -> Self {
        Self {
            ui_tx,
            cmd_rx,
            session_id: None,
        }
    }

    async fn run(mut self) {
        loop {
            let cmd = match self.cmd_rx.recv() {
                Ok(c) => c,
                Err(_) => break,
            };

            match cmd {
                AgentCommand::Start { cwd } => {
                    if let Err(e) = self.start_agent(cwd).await {
                        let _ = self.ui_tx.send(AgentUiEvent::ToolingError {
                            message: e.to_string(),
                        });
                    }
                }
                AgentCommand::Stop => break,
                _ => {}
            }
        }
    }

    async fn start_agent(&mut self, cwd: PathBuf) -> Result<()> {
        let mut child = Command::new("opencode")
            .arg("acp")
            .arg("--port")
            .arg("0")
            .current_dir(&cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| agent_client_protocol::Error::new(-32603, e.to_string()))?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let handler = TomeMessageHandler {
            ui_tx: self.ui_tx.clone(),
        };

        let (conn, io_fut) =
            ClientSideConnection::new(handler, stdin.compat_write(), stdout.compat(), |fut| {
                tokio::task::spawn_local(fut);
            });

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
                                .read_text_file(true)
                                .write_text_file(true))
                            .terminal(true),
                    )
                    .client_info(Implementation::new("Tome", "0.1.0").title("Tome Editor")),
            )
            .await?;

        let agent_info = init_res.agent_info.unwrap();
        let _ = self.ui_tx.send(AgentUiEvent::Connected {
            agent_name: agent_info.name,
            protocol_version: init_res.protocol_version.to_string(),
        });

        // New Session
        let session_res = conn.new_session(NewSessionRequest::new(cwd)).await?;

        self.session_id = Some(session_res.session_id.to_string());
        let _ = self.ui_tx.send(AgentUiEvent::SessionStarted {
            session_id: session_res.session_id.to_string(),
        });

        // Enter sub-loop for session commands
        self.session_loop(conn).await;

        Ok(())
    }

    async fn session_loop(&mut self, conn: Arc<ClientSideConnection>) {
        loop {
            let cmd = match self.cmd_rx.recv() {
                Ok(c) => c,
                Err(_) => break,
            };

            match cmd {
                AgentCommand::Prompt { content } => {
                    if let Some(session_id) = &self.session_id {
                        let req = PromptRequest::new(
                            session_id.clone(),
                            vec![ContentBlock::Text(TextContent::new(content))],
                        );
                        let conn_clone = conn.clone();
                        tokio::task::spawn_local(async move {
                            let _ = conn_clone.prompt(req).await;
                        });
                    }
                }
                AgentCommand::Cancel => {
                    if let Some(session_id) = &self.session_id {
                        let _ = conn
                            .cancel(CancelNotification::new(session_id.clone()))
                            .await;
                    }
                }
                AgentCommand::Stop => break,
                AgentCommand::Start { .. } => {
                    let _ = self.ui_tx.send(AgentUiEvent::ToolingError {
                        message: "Agent already started".to_string(),
                    });
                }
            }
        }
    }
}

#[derive(Clone)]
struct TomeMessageHandler {
    ui_tx: Sender<AgentUiEvent>,
}

impl MessageHandler<ClientSide> for TomeMessageHandler {
    fn handle_request(
        &self,
        request: AgentRequest,
    ) -> impl Future<Output = Result<ClientResponse>> {
        async move {
            match request {
                AgentRequest::ReadTextFileRequest(req) => {
                    let content = std::fs::read_to_string(&req.path)
                        .map_err(|e| agent_client_protocol::Error::new(-32000, e.to_string()))?;
                    Ok(ClientResponse::ReadTextFileResponse(
                        ReadTextFileResponse::new(content),
                    ))
                }
                AgentRequest::WriteTextFileRequest(req) => {
                    std::fs::write(&req.path, &req.content)
                        .map_err(|e| agent_client_protocol::Error::new(-32000, e.to_string()))?;
                    Ok(ClientResponse::WriteTextFileResponse(
                        WriteTextFileResponse::new(),
                    ))
                }
                AgentRequest::RequestPermissionRequest(req) => {
                    let outcome = RequestPermissionOutcome::Selected(
                        SelectedPermissionOutcome::new(req.options[0].option_id.clone()),
                    );
                    Ok(ClientResponse::RequestPermissionResponse(
                        RequestPermissionResponse::new(outcome),
                    ))
                }
                _ => Err(agent_client_protocol::Error::method_not_found()),
            }
        }
    }

    fn handle_notification(
        &self,
        notification: AgentNotification,
    ) -> impl Future<Output = Result<()>> {
        let ui_tx = self.ui_tx.clone();
        async move {
            match notification {
                AgentNotification::SessionNotification(sn) => {
                    let _ = ui_tx.send(AgentUiEvent::SessionUpdate(sn.update));
                }
                _ => {}
            }
            Ok(())
        }
    }
}
