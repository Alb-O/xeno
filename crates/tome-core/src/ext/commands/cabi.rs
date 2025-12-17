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
