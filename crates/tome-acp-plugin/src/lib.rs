use std::cell::RefCell;
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
use tokio::io::{AsyncBufReadExt, BufReader};
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

thread_local! {
    static PLUGIN: RefCell<Option<AcpPlugin>> = const { RefCell::new(None) };
}

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

enum AgentCommand {
    Start { cwd: PathBuf },
    Stop,
    Prompt { content: String },
    Cancel,
}

#[unsafe(no_mangle)]
/// # Safety
/// - `host` must be a valid pointer to a live `TomeHostV2` provided by the Tome host.
/// - `out_guest` must be a valid pointer to writable storage for a `TomeGuestV2`.
/// - Both pointers must remain valid for the duration of this call.
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

    PLUGIN.with(|ctx| {
        *ctx.borrow_mut() = Some(AcpPlugin {
            host,
            cmd_tx,
            events,
            panel_id,
            last_assistant_text,
        });
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
        reg(TomeCommandSpecV1 {
            name: tome_str("cancel"),
            aliases: TomeStrArray {
                ptr: std::ptr::null(),
                len: 0,
            },
            description: tome_str("Cancel the in-flight request"),
            handler: Some(command_cancel),
            user_data: std::ptr::null_mut(),
        });
    }

    TomeStatus::Ok
}

extern "C" fn plugin_shutdown() {
    PLUGIN.with(|ctx| {
        if let Some(plugin) = ctx.borrow_mut().take() {
            let _ = plugin.cmd_tx.try_send(AgentCommand::Stop);
        }
    });
}

extern "C" fn plugin_poll_event(out: *mut TomePluginEventV1) -> TomeBool {
    PLUGIN.with(|ctx| {
        if let Some(plugin) = ctx.borrow().as_ref() {
            let mut events = plugin.events.lock();
            if let Some(event) = events.pop_front() {
                unsafe { *out = event.0 };
                return TomeBool(1);
            }
        }
        TomeBool(0)
    })
}

extern "C" fn plugin_free_str(s: TomeOwnedStr) {
    if s.ptr.is_null() {
        return;
    }

    unsafe {
        let slice = std::ptr::slice_from_raw_parts_mut(s.ptr, s.len);
        drop(Box::from_raw(slice));
    }
}

extern "C" fn plugin_on_panel_submit(id: TomePanelId, text: TomeStr) {
    PLUGIN.with(|ctx| {
        if let Some(plugin) = ctx.borrow().as_ref() {
            let pid = plugin.panel_id.lock();
            if Some(id) == *pid {
                let s = tome_str_to_string(text);
                let _ = plugin.cmd_tx.try_send(AgentCommand::Prompt { content: s });
            }
        }
    });
}

extern "C" fn command_start(ctx: *mut TomeCommandContextV1) -> TomeStatus {
    PLUGIN.with(|p_ctx| {
        if let Some(plugin) = p_ctx.borrow().as_ref() {
            let host = unsafe { &*(*ctx).host };
            let mut cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

            if let (Some(get_path), Some(free_str)) = (host.get_current_path, host.free_str) {
                let mut owned_str = unsafe { std::mem::zeroed::<TomeOwnedStr>() };
                if get_path(&mut owned_str) == TomeStatus::Ok {
                    let path_str = tome_owned_to_string(owned_str);
                    free_str(owned_str);

                    if let Some(path_str) = path_str {
                        let path = PathBuf::from(path_str);
                        if let Some(parent) = path.parent() {
                            cwd = parent.to_path_buf();
                        }
                    }
                }
            }

            if !cwd.is_absolute()
                && let Ok(base) = std::env::current_dir()
            {
                cwd = base.join(cwd);
            }
            if let Ok(canon) = cwd.canonicalize() {
                cwd = canon;
            }

            let _ = plugin.cmd_tx.try_send(AgentCommand::Start { cwd });
            TomeStatus::Ok
        } else {
            TomeStatus::Failed
        }
    })
}

extern "C" fn command_stop(_ctx: *mut TomeCommandContextV1) -> TomeStatus {
    PLUGIN.with(|ctx| {
        if let Some(plugin) = ctx.borrow().as_ref() {
            let _ = plugin.cmd_tx.try_send(AgentCommand::Stop);
            TomeStatus::Ok
        } else {
            TomeStatus::Failed
        }
    })
}

extern "C" fn command_toggle(ctx: *mut TomeCommandContextV1) -> TomeStatus {
    PLUGIN.with(|p_ctx| {
        if let Some(plugin) = p_ctx.borrow().as_ref() {
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
    })
}

extern "C" fn command_insert_last(ctx: *mut TomeCommandContextV1) -> TomeStatus {
    PLUGIN.with(|p_ctx| {
        if let Some(plugin) = p_ctx.borrow().as_ref() {
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
    })
}

extern "C" fn command_cancel(_ctx: *mut TomeCommandContextV1) -> TomeStatus {
    PLUGIN.with(|ctx| {
        if let Some(plugin) = ctx.borrow().as_ref() {
            let _ = plugin.cmd_tx.try_send(AgentCommand::Cancel);
            TomeStatus::Ok
        } else {
            TomeStatus::Failed
        }
    })
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
                        let msg = format!("Failed to start agent: {e:?}");
                        self.enqueue_message(msg);
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
            // Do not inherit stderr: it can corrupt the TUI.
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| agent_client_protocol::Error::new(-32603, e.to_string()))?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take();

        let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        if let Some(stderr) = stderr {
            let events = self.events.clone();
            let panel_id = self.panel_id.clone();
            let stderr_tail = stderr_tail.clone();
            tokio::task::spawn_local(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = strip_ansi_and_controls(&line);
                    if line.trim().is_empty() {
                        continue;
                    }

                    {
                        let mut tail = stderr_tail.lock();
                        if tail.len() >= 50 {
                            tail.pop_front();
                        }
                        tail.push_back(line.clone());
                    }

                    enqueue_line(events.clone(), panel_id.clone(), format!("[acp] {}", line));
                }
            });
        }

        let handler = PluginMessageHandler {
            events: self.events.clone(),
            panel_id: self.panel_id.clone(),
            last_assistant_text: self.last_assistant_text.clone(),
        };

        let (conn, io_fut) =
            ClientSideConnection::new(handler, stdin.compat_write(), stdout.compat(), |fut| {
                tokio::task::spawn_local(fut);
            });

        let events = self.events.clone();
        let panel_id = self.panel_id.clone();
        tokio::task::spawn_local(async move {
            if let Err(e) = io_fut.await {
                enqueue_line(events, panel_id, format!("ACP IO error: {e:?}"));
            }
        });

        let conn = Arc::new(conn);

        let init_res = match conn
            .initialize(
                InitializeRequest::new(ProtocolVersion::from(1))
                    .client_capabilities(
                        ClientCapabilities::new()
                            .fs(FileSystemCapability::new()
                                .read_text_file(false)
                                .write_text_file(false))
                            .terminal(false),
                    )
                    .client_info(
                        Implementation::new("Tome-Plugin", "0.1.0").title("Tome ACP Plugin"),
                    ),
            )
            .await
        {
            Ok(res) => res,
            Err(e) => {
                let status = child
                    .try_wait()
                    .ok()
                    .flatten()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "(still running)".to_string());
                let stderr_tail = format_stderr_tail(&stderr_tail);
                let msg = format!(
                    "ACP initialize failed: err={e:?} child_status={status} stderr_tail={stderr_tail}"
                );
                self.enqueue_message(msg.clone());
                return Err(agent_client_protocol::Error::new(-32603, msg));
            }
        };

        let agent_info = init_res.agent_info.unwrap();
        self.enqueue_message(format!(
            "Connected to agent: {} (v{})",
            agent_info.name, init_res.protocol_version
        ));

        let session_res = match conn.new_session(NewSessionRequest::new(cwd.clone())).await {
            Ok(res) => res,
            Err(e) => {
                let status = child
                    .try_wait()
                    .ok()
                    .flatten()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "(still running)".to_string());
                let stderr_tail = format_stderr_tail(&stderr_tail);
                let msg = format!(
                    "ACP new_session failed: cwd={cwd:?} err={e:?} child_status={status} stderr_tail={stderr_tail}"
                );
                self.enqueue_message(msg.clone());
                return Err(agent_client_protocol::Error::new(-32603, msg));
            }
        };
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
        enqueue_line(self.events.clone(), self.panel_id.clone(), msg);
    }
}

fn enqueue_line(
    events: Arc<Mutex<VecDeque<SendEvent>>>,
    panel_id: Arc<Mutex<Option<TomePanelId>>>,
    msg: String,
) {
    let msg = strip_ansi_and_controls(&msg);

    let mut events = events.lock();
    if let Some(pid) = *panel_id.lock() {
        events.push_back(SendEvent(TomePluginEventV1 {
            kind: TomePluginEventKind::PanelAppend,
            panel_id: pid,
            role: TomeChatRole::System,
            text: string_to_tome_owned(msg),
            bool_val: TomeBool(0),
            permission_request_id: 0,
            permission_request: std::ptr::null_mut(),
        }));
    } else {
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

fn format_stderr_tail(stderr_tail: &Mutex<VecDeque<String>>) -> String {
    let tail = stderr_tail.lock();
    if tail.is_empty() {
        return "(empty)".to_string();
    }

    let mut joined = tail.iter().cloned().collect::<Vec<_>>().join(" | ");
    const MAX_LEN: usize = 800;
    if joined.len() > MAX_LEN {
        joined.truncate(MAX_LEN);
        joined.push_str("...");
    }
    format!("\"{}\"", joined)
}

fn strip_ansi_and_controls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            // Drop CSI escape sequences (ESC [ ... <final byte>).
            if matches!(chars.peek(), Some('[')) {
                let _ = chars.next();
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }

        if ch.is_control() {
            continue;
        }

        out.push(ch);
    }

    out
}

struct PluginMessageHandler {
    events: Arc<Mutex<VecDeque<SendEvent>>>,
    panel_id: Arc<Mutex<Option<TomePanelId>>>,
    last_assistant_text: Arc<Mutex<String>>,
}

impl MessageHandler<ClientSide> for PluginMessageHandler {
    #[allow(clippy::manual_async_fn)]
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
            if let AgentNotification::SessionNotification(sn) = notification {
                handle_session_update(sn.update, events, panel_id, last_text);
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

fn tome_owned_to_string(tos: TomeOwnedStr) -> Option<String> {
    if tos.ptr.is_null() {
        return None;
    }

    unsafe {
        let slice = std::slice::from_raw_parts(tos.ptr, tos.len);
        Some(String::from_utf8_lossy(slice).into_owned())
    }
}

fn string_to_tome_owned(s: String) -> TomeOwnedStr {
    let bytes = s.into_bytes().into_boxed_slice();
    let len = bytes.len();
    let ptr = Box::into_raw(bytes) as *mut u8;
    TomeOwnedStr { ptr, len }
}
