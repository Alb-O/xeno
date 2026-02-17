use crate::runtime::work_queue::{RuntimeWorkSource, WorkScope};
use crate::types::{Invocation, InvocationOutcome, InvocationPolicy};

/// Typed invocation command envelope consumed by invocation runtime boundaries.
#[derive(Debug, Clone)]
pub enum InvocationCmd {
	Run {
		invocation: Invocation,
		policy: InvocationPolicy,
		source: RuntimeWorkSource,
		scope: WorkScope,
		seq: u64,
	},
}

/// Typed invocation event envelope emitted after invocation command execution.
#[derive(Debug, Clone)]
pub enum InvocationEvt {
	Completed(InvocationOutcome),
}
