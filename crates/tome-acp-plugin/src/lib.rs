use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use agent_client_protocol::{
    Agent, AgentNotification, AgentRequest, CancelNotification, ClientCapabilities, ClientResponse,
    ClientSide, ClientSideConnection, ContentBlock, FileSystemCapability, Implementation,
    InitializeRequest, MessageHandler, NewSessionRequest, PromptRequest, ProtocolVersion,
    ReadTextFileResponse, RequestPermissionOutcome, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionUpdate, TextContent, WriteTextFileResponse,
};
use parking_lot::Mutex;
use tokio::process::Command;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::LocalSet;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tome_cabi_types::{
    TOME_C_ABI_VERSION_V2, TomeBool, TomeChatRole, TomeCommandContextV1, TomeCommandSpecV1,
    TomeGuestV2, TomeHostV2, TomeOwnedStr, TomePanelId, TomePanelKind, TomePluginEventKind,
    TomePluginEventV1, TomeStatus, TomeStr, TomeStrArray,
};

static PLUGIN: Mutex<Option<AcpPlugin>> = Mutex::new(None);

struct AcpPlugin {
    #[allow(dead_code)]
    host: *const TomeHostV2,
    cmd_tx: Sender<AgentCommand>,
    events: Arc<Mutex<VecDeque<SendEvent>>>,
    panel_id: Arc<Mutex<Option<TomePanelId>>>,
    last_assistant_text: Arc<Mutex<String>>,
}

struct SendEvent(TomePluginEventV1);
unsafe impl Send for SendEvent {}
unsafe impl Sync for SendEvent {}

unsafe impl Send for AcpPlugin {}
unsafe impl Sync for AcpPlugin {}

enum AgentCommand {
    Start { cwd: PathBuf },
    Stop,
    Prompt { content: String },
    Cancel,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tome_plugin_entry_v2(
    host: *const TomeHostV2,
    out_guest: *mut TomeGuestV2,
) -> TomeStatus {
    if host.is_null() || out_guest.is_null() {
        return TomeStatus::Failed;
    }

    let host_ref = unsafe { &*host };
    if host_ref.abi_version != TOME_C_ABI_VERSION_V2 {
        return TomeStatus::Incompatible;
    }

    unsafe {
        *out_guest = TomeGuestV2 {
            abi_version: TOME_C_ABI_VERSION_V2,
            namespace: tome_str("acp"),
            name: tome_str("ACP Agent"),
            version: tome_str("0.1.0"),
            init: Some(plugin_init),
            shutdown: Some(plugin_shutdown),
            poll_event: Some(plugin_poll_event),
            free_str: Some(plugin_free_str),
            on_panel_submit: Some(plugin_on_panel_submit),
            on_permission_decision: None,
        };
    }

    TomeStatus::Ok
}

extern "C" fn plugin_init(host: *const TomeHostV2) -> TomeStatus {
    let (cmd_tx, cmd_rx) = mpsc::channel(100);
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let panel_id = Arc::new(Mutex::new(None));
    let last_assistant_text = Arc::new(Mutex::new(String::new()));

    let events_clone = events.clone();
    let panel_id_clone = panel_id.clone();
    let last_text_clone = last_assistant_text.clone();

    thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        let local = LocalSet::new();

        local.block_on(&rt, async {
            AcpBackend::new(cmd_rx, events_clone, panel_id_clone, last_text_clone)
                .run()
                .await;
        });
    });

    let mut plugin_guard = PLUGIN.lock();
    *plugin_guard = Some(AcpPlugin {
        host,
        cmd_tx,
        events,
        panel_id,
        last_assistant_text,
    });

    let host_ref = unsafe { &*host };
    if let Some(reg) = host_ref.register_command {
        reg(TomeCommandSpecV1 {
            name: tome_str("start"),
            aliases: TomeStrArray {
                ptr: std::ptr::null(),
                len: 0,
            },
            description: tome_str("Start the ACP agent"),
            handler: Some(command_start),
            user_data: std::ptr::null_mut(),
        });
        reg(TomeCommandSpecV1 {
            name: tome_str("stop"),
            aliases: TomeStrArray {
                ptr: std::ptr::null(),
                len: 0,
            },
            description: tome_str("Stop the ACP agent"),
            handler: Some(command_stop),
            user_data: std::ptr::null_mut(),
        });
        reg(TomeCommandSpecV1 {
            name: tome_str("toggle"),
            aliases: TomeStrArray {
                ptr: std::ptr::null(),
                len: 0,
            },
            description: tome_str("Toggle the ACP agent panel"),
            handler: Some(command_toggle),
            user_data: std::ptr::null_mut(),
        });
        reg(TomeCommandSpecV1 {
            name: tome_str("insert_last"),
            aliases: TomeStrArray {
                ptr: std::ptr::null(),
                len: 0,
            },
            description: tome_str("Insert last response"),
            handler: Some(command_insert_last),
            user_data: std::ptr::null_mut(),
        });
    }

    TomeStatus::Ok
}

extern "C" fn plugin_shutdown() {
    let mut plugin_guard = PLUGIN.lock();
    if let Some(plugin) = plugin_guard.take() {
        let _ = plugin.cmd_tx.try_send(AgentCommand::Stop);
    }
}

extern "C" fn plugin_poll_event(out: *mut TomePluginEventV1) -> TomeBool {
    let plugin_guard = PLUGIN.lock();
    if let Some(plugin) = plugin_guard.as_ref() {
        let mut events = plugin.events.lock();
        if let Some(event) = events.pop_front() {
            unsafe { *out = event.0 };
            return TomeBool(1);
        }
    }
    TomeBool(0)
}

extern "C" fn plugin_free_str(s: TomeOwnedStr) {
    if !s.ptr.is_null() {
        unsafe {
            let _ = Vec::from_raw_parts(s.ptr, s.len, s.len);
        }
    }
}

extern "C" fn plugin_on_panel_submit(id: TomePanelId, text: TomeStr) {
    let plugin_guard = PLUGIN.lock();
    if let Some(plugin) = plugin_guard.as_ref() {
        let pid = plugin.panel_id.lock();
        if Some(id) == *pid {
            let s = tome_str_to_string(text);
            let _ = plugin.cmd_tx.try_send(AgentCommand::Prompt { content: s });
        }
    }
}

extern "C" fn command_start(_ctx: *mut TomeCommandContextV1) -> TomeStatus {
    let plugin_guard = PLUGIN.lock();
    if let Some(plugin) = plugin_guard.as_ref() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let _ = plugin.cmd_tx.try_send(AgentCommand::Start { cwd });
        TomeStatus::Ok
    } else {
        TomeStatus::Failed
    }
}

extern "C" fn command_stop(_ctx: *mut TomeCommandContextV1) -> TomeStatus {
    let plugin_guard = PLUGIN.lock();
    if let Some(plugin) = plugin_guard.as_ref() {
        let _ = plugin.cmd_tx.try_send(AgentCommand::Stop);
        TomeStatus::Ok
    } else {
        TomeStatus::Failed
    }
}

extern "C" fn command_toggle(ctx: *mut TomeCommandContextV1) -> TomeStatus {
    let plugin_guard = PLUGIN.lock();
    if let Some(plugin) = plugin_guard.as_ref() {
        let host = unsafe { &*(*ctx).host };
        let mut pid_guard = plugin.panel_id.lock();
        let pid = match *pid_guard {
            Some(id) => id,
            None => {
                let id = (host.panel.create)(TomePanelKind::Chat, tome_str("ACP Agent"));
                *pid_guard = Some(id);
                id
            }
        };
        (host.panel.set_open)(pid, TomeBool(1));
        (host.panel.set_focused)(pid, TomeBool(1));
        TomeStatus::Ok
    } else {
        TomeStatus::Failed
    }
}

extern "C" fn command_insert_last(ctx: *mut TomeCommandContextV1) -> TomeStatus {
    let plugin_guard = PLUGIN.lock();
    if let Some(plugin) = plugin_guard.as_ref() {
        let text = plugin.last_assistant_text.lock().clone();
        if !text.is_empty() {
            let host = unsafe { &*(*ctx).host };
            let ts = TomeStr {
                ptr: text.as_ptr(),
                len: text.len(),
            };
            (host.insert_text)(ts);
            TomeStatus::Ok
        } else {
            TomeStatus::Failed
        }
    } else {
        TomeStatus::Failed
    }
}

struct AcpBackend {
    cmd_rx: Receiver<AgentCommand>,
    events: Arc<Mutex<VecDeque<SendEvent>>>,
    panel_id: Arc<Mutex<Option<TomePanelId>>>,
    last_assistant_text: Arc<Mutex<String>>,
    session_id: Option<String>,
}

impl AcpBackend {
    fn new(
        cmd_rx: Receiver<AgentCommand>,
        events: Arc<Mutex<VecDeque<SendEvent>>>,
        panel_id: Arc<Mutex<Option<TomePanelId>>>,
        last_assistant_text: Arc<Mutex<String>>,
    ) -> Self {
        Self {
            cmd_rx,
            events,
            panel_id,
            last_assistant_text,
            session_id: None,
        }
    }

    async fn run(mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                AgentCommand::Start { cwd } => {
                    if let Err(e) = self.start_agent(cwd).await {
                        self.enqueue_message(format!("Failed to start agent: {}", e));
                    }
                }
                AgentCommand::Stop => break,
                _ => {}
            }
        }
    }

    async fn start_agent(&mut self, cwd: PathBuf) -> agent_client_protocol::Result<()> {
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

        let handler = PluginMessageHandler {
            events: self.events.clone(),
            panel_id: self.panel_id.clone(),
            last_assistant_text: self.last_assistant_text.clone(),
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
                    .client_info(
                        Implementation::new("Tome-Plugin", "0.1.0").title("Tome ACP Plugin"),
                    ),
            )
            .await?;

        let agent_info = init_res.agent_info.unwrap();
        self.enqueue_message(format!(
            "Connected to agent: {} (v{})",
            agent_info.name, init_res.protocol_version
        ));

        let session_res = conn.new_session(NewSessionRequest::new(cwd)).await?;
        self.session_id = Some(session_res.session_id.to_string());
        self.enqueue_message("Session started".to_string());

        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                AgentCommand::Prompt { content } => {
                    {
                        let mut last = self.last_assistant_text.lock();
                        last.clear();
                    }
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
                    self.enqueue_message("Agent already started".to_string());
                }
            }
        }

        Ok(())
    }

    fn enqueue_message(&self, msg: String) {
        let mut events = self.events.lock();
        events.push_back(SendEvent(TomePluginEventV1 {
            kind: TomePluginEventKind::ShowMessage,
            panel_id: 0,
            role: TomeChatRole::System,
            text: string_to_tome_owned(msg),
            bool_val: TomeBool(0),
            permission_request_id: 0,
            permission_request: std::ptr::null_mut(),
        }));
    }
}

struct PluginMessageHandler {
    events: Arc<Mutex<VecDeque<SendEvent>>>,
    panel_id: Arc<Mutex<Option<TomePanelId>>>,
    last_assistant_text: Arc<Mutex<String>>,
}

impl MessageHandler<ClientSide> for PluginMessageHandler {
    fn handle_request(
        &self,
        request: AgentRequest,
    ) -> impl std::future::Future<Output = agent_client_protocol::Result<ClientResponse>> {
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
    ) -> impl std::future::Future<Output = agent_client_protocol::Result<()>> {
        let events = self.events.clone();
        let panel_id = self.panel_id.clone();
        let last_text = self.last_assistant_text.clone();
        async move {
            match notification {
                AgentNotification::SessionNotification(sn) => {
                    handle_session_update(sn.update, events, panel_id, last_text);
                }
                _ => {}
            }
            Ok(())
        }
    }
}

fn handle_session_update(
    update: SessionUpdate,
    events: Arc<Mutex<VecDeque<SendEvent>>>,
    panel_id: Arc<Mutex<Option<TomePanelId>>>,
    last_text: Arc<Mutex<String>>,
) {
    let pid = match *panel_id.lock() {
        Some(id) => id,
        None => return,
    };

    match update {
        SessionUpdate::AgentMessageChunk(chunk) => {
            if let ContentBlock::Text(text) = chunk.content {
                {
                    let mut last = last_text.lock();
                    last.push_str(&text.text);
                }
                let mut e = events.lock();
                e.push_back(SendEvent(TomePluginEventV1 {
                    kind: TomePluginEventKind::PanelAppend,
                    panel_id: pid,
                    role: TomeChatRole::Assistant,
                    text: string_to_tome_owned(text.text),
                    bool_val: TomeBool(0),
                    permission_request_id: 0,
                    permission_request: std::ptr::null_mut(),
                }));
            }
        }
        SessionUpdate::AgentThoughtChunk(chunk) => {
            if let ContentBlock::Text(text) = chunk.content {
                let mut e = events.lock();
                e.push_back(SendEvent(TomePluginEventV1 {
                    kind: TomePluginEventKind::PanelAppend,
                    panel_id: pid,
                    role: TomeChatRole::Thought,
                    text: string_to_tome_owned(text.text),
                    bool_val: TomeBool(0),
                    permission_request_id: 0,
                    permission_request: std::ptr::null_mut(),
                }));
            }
        }
        _ => {}
    }
}

fn tome_str(s: &'static str) -> TomeStr {
    TomeStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

fn tome_str_to_string(ts: TomeStr) -> String {
    if ts.ptr.is_null() {
        return String::new();
    }
    unsafe {
        let slice = std::slice::from_raw_parts(ts.ptr, ts.len);
        String::from_utf8_lossy(slice).into_owned()
    }
}

fn string_to_tome_owned(s: String) -> TomeOwnedStr {
    let mut b = s.into_bytes();
    let ptr = b.as_mut_ptr();
    let len = b.len();
    std::mem::forget(b);
    TomeOwnedStr { ptr, len }
}
