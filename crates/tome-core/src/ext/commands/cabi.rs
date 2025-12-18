use std::path::Path;
use std::sync::Mutex;

use linkme::distributed_slice;

use crate::ext::plugins::{CAbiLoadError, load_c_abi_plugin};
use crate::ext::{COMMANDS, CommandContext, CommandDef, CommandError, CommandOutcome};

static LOADED_C_ABI_PLUGINS: Mutex<Vec<crate::ext::plugins::CAbiPlugin>> = Mutex::new(Vec::new());

#[distributed_slice(COMMANDS)]
static CMD_CABI_LOAD: CommandDef = CommandDef {
    name: "cabi_load",
    aliases: &[],
    description: "Load a C-ABI plugin (.so/.dylib/.dll)",
    handler: cmd_cabi_load,
};

fn cmd_cabi_load(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    let Some(&path) = ctx.args.first() else {
        return Err(CommandError::MissingArgument("path"));
    };

    match load_c_abi_plugin(Path::new(path)) {
        Ok(plugin) => {
            LOADED_C_ABI_PLUGINS
                .lock()
                .map_err(|_| CommandError::Failed("plugin lock poisoned".into()))?
                .push(plugin);
            ctx.message(&format!("Loaded C-ABI plugin from {path}"));
            Ok(CommandOutcome::Ok)
        }
        Err(CAbiLoadError::InitFailed) => Err(CommandError::Failed("plugin init failed".into())),
        Err(CAbiLoadError::Incompatible { host, guest }) => Err(CommandError::Failed(format!(
            "plugin ABI mismatch: host={} guest={}",
            host, guest
        ))),
        Err(CAbiLoadError::MissingEntry) => Err(CommandError::Failed(
            "missing entry symbol tome_plugin_entry".into(),
        )),
        Err(CAbiLoadError::Load(e)) => Err(CommandError::Failed(format!("dlopen failed: {e}"))),
    }
}

#[distributed_slice(COMMANDS)]
static CMD_PERMIT: CommandDef = CommandDef {
    name: "permit",
    aliases: &[],
    description: "Respond to a plugin permission request (:permit <id> <option>)",
    handler: cmd_permit,
};

fn cmd_permit(ctx: &mut CommandContext) -> Result<CommandOutcome, CommandError> {
    let Some(&id_str) = ctx.args.first() else {
        return Err(CommandError::MissingArgument("id"));
    };
    let Some(&option) = ctx.args.get(1) else {
        return Err(CommandError::MissingArgument("option"));
    };

    let id: u64 = id_str
        .parse()
        .map_err(|_| CommandError::InvalidArgument("id must be a number".into()))?;

    ctx.editor
        .on_permission_decision(id, option)
        .map_err(CommandError::Failed)?;

    Ok(CommandOutcome::Ok)
}
