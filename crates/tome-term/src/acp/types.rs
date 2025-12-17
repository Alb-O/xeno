use tome_core::Rope;

#[derive(Debug, Default)]
pub struct AgentPanelState {
    pub open: bool,
    pub focused: bool,
    pub input: Rope,
    pub input_cursor: usize,
    pub transcript: Vec<ChatItem>,
    pub last_assistant_text: String,
}

#[derive(Debug)]
pub enum ChatItem {
    User(String),
    Assistant(String),
    Thought(String),
}
