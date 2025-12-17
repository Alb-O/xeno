use crate::ext::{COMMANDS, CommandDef, CommandOutcome};
use linkme::distributed_slice;

#[distributed_slice(COMMANDS)]
pub static CMD_AGENT: CommandDef = CommandDef {
    name: "agent",
    aliases: &[],
    description: "Toggle the agent panel",
    handler: |ctx| {
        ctx.editor.agent_toggle();
        Ok(CommandOutcome::Ok)
    },
};

#[distributed_slice(COMMANDS)]
pub static CMD_AGENT_START: CommandDef = CommandDef {
    name: "agent_start",
    aliases: &[],
    description: "Start the agent",
    handler: |ctx| {
        ctx.editor.agent_start();
        Ok(CommandOutcome::Ok)
    },
};

#[distributed_slice(COMMANDS)]
pub static CMD_AGENT_STOP: CommandDef = CommandDef {
    name: "agent_stop",
    aliases: &[],
    description: "Stop the agent",
    handler: |ctx| {
        ctx.editor.agent_stop();
        Ok(CommandOutcome::Ok)
    },
};

#[distributed_slice(COMMANDS)]
pub static CMD_AGENT_INSERT: CommandDef = CommandDef {
    name: "agent_insert",
    aliases: &[],
    description: "Insert last assistant message",
    handler: |ctx| {
        ctx.editor.agent_insert_last();
        Ok(CommandOutcome::Ok)
    },
};
