//! Built-in command registrations for command-line mode.
//!
//! These are the `:commands` that users can invoke from the command prompt.
//! Commands now use the `EditorOps` trait to perform real operations.

use linkme::distributed_slice;

use super::{CommandContext, CommandDef, CommandError, CommandOutcome, COMMANDS};

#[distributed_slice(COMMANDS)]
static CMD_HELP: CommandDef = CommandDef {
    name: "help",
    aliases: &["h"],
    description: "Show help for commands",
    handler: cmd_help,
};

fn cmd_help(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    let help_text: Vec<String> = super::COMMANDS
        .iter()
        .map(|c| {
            let aliases = if c.aliases.is_empty() {
                String::new()
            } else {
                format!(" ({})", c.aliases.join(", "))
            };
            format!(":{}{} - {}", c.name, aliases, c.description)
        })
        .collect();
    ctx.message(&help_text.join(" | "));
    Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_QUIT: CommandDef = CommandDef {
    name: "quit",
    aliases: &["q"],
    description: "Quit the editor",
    handler: cmd_quit,
};

fn cmd_quit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    if ctx.editor.is_modified() {
        ctx.error("Buffer has unsaved changes (use :q! to force quit)");
        return Ok(CommandOutcome::Ok);
    }
    Ok(CommandOutcome::Quit)
}

#[distributed_slice(COMMANDS)]
static CMD_QUIT_FORCE: CommandDef = CommandDef {
    name: "quit!",
    aliases: &["q!"],
    description: "Quit without saving",
    handler: cmd_quit_force,
};

fn cmd_quit_force(_ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    Ok(CommandOutcome::ForceQuit)
}

#[distributed_slice(COMMANDS)]
static CMD_WRITE: CommandDef = CommandDef {
    name: "write",
    aliases: &["w"],
    description: "Write buffer to file",
    handler: cmd_write,
};

fn cmd_write(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    ctx.editor.save()?;
    Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_WRITE_QUIT: CommandDef = CommandDef {
    name: "wq",
    aliases: &["x"],
    description: "Write and quit",
    handler: cmd_write_quit,
};

fn cmd_write_quit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    ctx.editor.save()?;
    Ok(CommandOutcome::Quit)
}

#[distributed_slice(COMMANDS)]
static CMD_EDIT: CommandDef = CommandDef {
    name: "edit",
    aliases: &["e"],
    description: "Edit a file",
    handler: cmd_edit,
};

fn cmd_edit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    if ctx.args.is_empty() {
        return Err(CommandError::MissingArgument("filename"));
    }
    ctx.message(&format!("edit {} - not yet implemented", ctx.args[0]));
    Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_BUFFER: CommandDef = CommandDef {
    name: "buffer",
    aliases: &["b"],
    description: "Switch to buffer",
    handler: cmd_buffer,
};

fn cmd_buffer(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    if ctx.args.is_empty() {
        return Err(CommandError::MissingArgument("buffer name or number"));
    }
    ctx.message(&format!("buffer {} - not yet implemented", ctx.args[0]));
    Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_BUFFER_NEXT: CommandDef = CommandDef {
    name: "buffer-next",
    aliases: &["bn"],
    description: "Go to next buffer",
    handler: cmd_buffer_next,
};

fn cmd_buffer_next(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    ctx.message("buffer-next - not yet implemented");
    Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_BUFFER_PREV: CommandDef = CommandDef {
    name: "buffer-previous",
    aliases: &["bp"],
    description: "Go to previous buffer",
    handler: cmd_buffer_prev,
};

fn cmd_buffer_prev(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    ctx.message("buffer-previous - not yet implemented");
    Ok(CommandOutcome::Ok)
}

#[distributed_slice(COMMANDS)]
static CMD_DELETE_BUFFER: CommandDef = CommandDef {
    name: "delete-buffer",
    aliases: &["db"],
    description: "Delete current buffer",
    handler: cmd_delete_buffer,
};

fn cmd_delete_buffer(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    ctx.message("delete-buffer - not yet implemented");
    Ok(CommandOutcome::Ok)
}
