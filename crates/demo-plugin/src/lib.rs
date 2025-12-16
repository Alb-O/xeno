use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PluginRegistration {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub actions: Vec<ActionRegistration>,
    #[serde(default)]
    pub commands: Vec<CommandRegistration>,
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub keybindings: Vec<PluginKeybinding>,
}

#[derive(Serialize, Deserialize)]
pub struct ActionRegistration {
    pub name: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct CommandRegistration {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct PluginKeybinding {
    pub mode: String,
    pub key: String,
    pub action: String,
}

#[derive(Serialize, Deserialize)]
pub struct ActionInput {
    pub action_name: String,
    pub count: usize,
    pub extend: bool,
    pub char_arg: Option<char>,
    pub editor: EditorState,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ActionOutput {
    #[serde(default)]
    pub set_cursor: Option<usize>,
    #[serde(default)]
    pub set_selection: Option<(usize, usize)>,
    #[serde(default)]
    pub insert_text: Option<String>,
    #[serde(default)]
    pub delete: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct EditorState {
    pub text: String,
    pub cursor: usize,
    pub selection_anchor: usize,
    pub selection_head: usize,
}

#[derive(Serialize, Deserialize)]
pub struct CommandInput {
    pub command_name: String,
    pub args: Vec<String>,
    pub editor: EditorState,
}

#[derive(Serialize, Deserialize)]
pub struct HookInput {
    pub hook_name: String,
    pub editor: EditorState,
    #[serde(default)]
    pub extra: serde_json::Value,
}

#[plugin_fn]
pub fn plugin_init() -> FnResult<Json<PluginRegistration>> {
    Ok(Json(PluginRegistration {
        name: "Demo Plugin".to_string(),
        version: "0.1.0".to_string(),
        actions: vec![
            ActionRegistration {
                name: "demo_insert_hello".to_string(),
                description: "Insert 'Hello from Plugin!'".to_string(),
            },
            ActionRegistration {
                name: "demo_upper_selection".to_string(),
                description: "Uppercase selection".to_string(),
            },
        ],
        commands: vec![
            CommandRegistration {
                name: "hello".to_string(),
                aliases: vec![],
                description: "Say hello via command".to_string(),
            }
        ],
        hooks: vec![],
        keybindings: vec![],
    }))
}

#[plugin_fn]
pub fn on_action(Json(input): Json<ActionInput>) -> FnResult<Json<ActionOutput>> {
    match input.action_name.as_str() {
        "demo_insert_hello" => {
            Ok(Json(ActionOutput {
                insert_text: Some("Hello from Plugin!".to_string()),
                message: Some("Executed demo action".to_string()),
                ..Default::default()
            }))
        }
        "demo_upper_selection" => {
            let text = &input.editor.text;
            let anchor = input.editor.selection_anchor;
            let head = input.editor.selection_head;
            
            let (from, to) = if anchor < head { (anchor, head) } else { (head, anchor) };
            
            if from < to && to <= text.len() {
                let selected = &text[from..to];
                let upper = selected.to_uppercase();
                
                Ok(Json(ActionOutput {
                    insert_text: Some(upper),
                    message: Some("Uppercased selection".to_string()),
                    ..Default::default()
                }))
            } else {
                Ok(Json(ActionOutput {
                    message: Some("No selection".to_string()),
                    ..Default::default()
                }))
            }
        }
        _ => Ok(Json(ActionOutput::default())),
    }
}

#[plugin_fn]
pub fn on_command(Json(input): Json<CommandInput>) -> FnResult<Json<ActionOutput>> {
    match input.command_name.as_str() {
        "hello" => {
             Ok(Json(ActionOutput {
                insert_text: Some("Hello from Command!".to_string()),
                message: Some("Ran hello command".to_string()),
                ..Default::default()
            }))
        }
        _ => Ok(Json(ActionOutput::default())),
    }
}

#[plugin_fn]
pub fn on_hook(Json(_input): Json<HookInput>) -> FnResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({})))
}
