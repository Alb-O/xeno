//! Actor mailbox primitives and overflow behavior.

pub use crate::mailbox::{
	Mailbox as ActorMailbox, MailboxPolicy as ActorMailboxMode, MailboxReceiver as ActorMailboxReceiver, MailboxSendError as ActorMailboxSendError,
	MailboxSendOutcome as ActorMailboxSendOutcome, MailboxSender as ActorMailboxSender,
};

/// Mailbox configuration for actor specs.
pub type ActorMailboxPolicy = crate::supervisor::MailboxSpec;
