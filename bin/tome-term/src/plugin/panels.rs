use tome_cabi_types::TomeChatRole;
use tome_core::Rope;

#[derive(Debug)]
pub struct ChatPanelState {
	pub id: u64,
	pub title: String,
	pub open: bool,
	pub focused: bool,
	pub input: Rope,
	pub input_cursor: usize,
	pub transcript: Vec<ChatItem>,
}

#[derive(Debug)]
pub struct ChatItem {
	pub role: TomeChatRole,
	pub text: String,
}

impl ChatPanelState {
	pub fn new(id: u64, title: String) -> Self {
		Self {
			id,
			title,
			open: false,
			focused: false,
			input: Rope::from(""),
			input_cursor: 0,
			transcript: Vec::new(),
		}
	}
}
