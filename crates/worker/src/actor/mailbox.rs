//! Actor mailbox primitives.

pub use crate::mailbox::{
	Mailbox as ActorMailbox, MailboxReceiver as ActorMailboxReceiver, MailboxSendError as ActorMailboxSendError,
	MailboxSender as ActorMailboxSender,
};

/// Mailbox configuration for actor specs.
pub type ActorMailboxPolicy = crate::supervisor::MailboxSpec;
