use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use kdl::{KdlDocument, KdlNode};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

const MAGIC: &[u8; 8] = b"XENOASST";
const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct LanguageDataRaw {
    name: String,
    grammar_name: String,
    extensions: Vec<String>,
    filenames: Vec<String>,
    globs: Vec<String>,
    shebangs: Vec<String>,
    comment_tokens: Vec<String>,
    block_comment: Option<(String, String)>,
    injection_regex: Option<String>,
    lsp_servers: Vec<String>,
    roots: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct LspServerDefRaw {
    name: String,
    command: String,
    args: Vec<String>,
    environment: HashMap<String, String>,
    config_json: Option<String>,
    source: Option<String>,
    nix: Option<String>,
}

fn write_blob(path: &PathBuf, data: &[u8]) {
    let mut file = fs::File::create(path).expect("failed to create blob");
    file.write_all(MAGIC).expect("failed to write magic");
    file.write_all(&SCHEMA_VERSION.to_le_bytes())
        .expect("failed to write version");
    file.write_all(data).expect("failed to write data");
}

fn parse_string_args(children: Option<&KdlDocument>, node_name: &str) -> Vec<String> {
    let Some(children) = children else {
        return Vec::new();
    };
    let Some(node) = children.get(node_name) else {
        return Vec::new();
    };
    node.entries()
        .iter()
        .filter(|e| e.name().is_none())
        .filter_map(|e| e.value().as_string())
        .map(String::from)
        .collect()
}

fn parse_file_types(children: Option<&KdlDocument>) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut extensions = Vec::new();
    let mut filenames = Vec::new();
    let mut globs = Vec::new();

    let Some(children) = children else {
        return (extensions, filenames, globs);
    };
    let Some(file_types_node) = children.get("file-types") else {
        return (extensions, filenames, globs);
    };

    for entry in file_types_node.entries() {
        if entry.name().is_none()
            && let Some(s) = entry.value().as_string() {
                extensions.push(s.to_string());
            }
    }

    if let Some(ft_children) = file_types_node.children() {
        for child in ft_children.nodes() {
            if child.name().value() != "-" {
                continue;
            }
            if let Some(glob) = child.get("glob").and_then(|v| v.as_string()) {
                if glob.contains('*') || glob.contains('?') || glob.contains('[') {
                    globs.push(glob.to_string());
                } else {
                    filenames.push(glob.to_string());
                }
            } else if let Some(s) = child.entry(0).and_then(|e| e.value().as_string()) {
                extensions.push(s.to_string());
            }
        }
    }

    (extensions, filenames, globs)
}

fn parse_block_comment(
    node: &KdlNode,
    children: Option<&KdlDocument>,
) -> Option<(String, String)> {
    if let (Some(start), Some(end)) = (
        node.get("block-comment-start").and_then(|v| v.as_string()),
        node.get("block-comment-end").and_then(|v| v.as_string()),
    ) {
        return Some((start.to_string(), end.to_string()));
    }

    let bc_node = children?.get("block-comment-tokens")?;

    if let (Some(start), Some(end)) = (
        bc_node.get("start").and_then(|v| v.as_string()),
        bc_node.get("end").and_then(|v| v.as_string()),
    ) {
        return Some((start.to_string(), end.to_string()));
    }

    bc_node.children().and_then(|bc_children| {
        bc_children.nodes().iter().find_map(|child| {
            if child.name().value() != "-" {
                return None;
            }
            let start = child.get("start").and_then(|v| v.as_string())?;
            let end = child.get("end").and_then(|v| v.as_string())?;
            Some((start.to_string(), end.to_string()))
        })
    })
}

fn parse_language_servers_from_lang(children: Option<&KdlDocument>) -> Vec<String> {
    let Some(children) = children else {
        return Vec::new();
    };
    let Some(ls_node) = children.get("language-servers") else {
        return Vec::new();
    };

    let inline: Vec<String> = ls_node
        .entries()
        .iter()
        .filter(|e| e.name().is_none())
        .filter_map(|e| e.value().as_string())
        .map(String::from)
        .collect();

    if !inline.is_empty() {
        return inline;
    }

    let Some(ls_children) = ls_node.children() else {
        return Vec::new();
    };

    ls_children
        .nodes()
        .iter()
        .filter(|n| n.name().value() == "-")
        .filter_map(|n| n.get("name").and_then(|v| v.as_string()).map(String::from))
        .collect()
}

fn parse_language_node(node: &KdlNode) -> Option<LanguageDataRaw> {
    let name = node.get("name").and_then(|v| v.as_string())?.to_string();
    let grammar = node
        .get("grammar")
        .and_then(|v| v.as_string())
        .map(String::from)
        .unwrap_or_else(|| name.clone());
    let injection_regex = node
        .get("injection-regex")
        .and_then(|v| v.as_string())
        .map(String::from);

    let children = node.children();
    let (extensions, filenames, globs) = parse_file_types(children);
    let shebangs = parse_string_args(children, "shebangs");

    let mut comment_tokens = Vec::new();
    if let Some(token) = node.get("comment-token").and_then(|v| v.as_string()) {
        comment_tokens.push(token.to_string());
    }
    comment_tokens.extend(parse_string_args(children, "comment-tokens"));

    let block_comment = parse_block_comment(node, children);
    let lsp_servers = parse_language_servers_from_lang(children);
    let roots = parse_string_args(children, "roots");

    Some(LanguageDataRaw {
        name,
        grammar_name: grammar,
        extensions,
        filenames,
        globs,
        shebangs,
        comment_tokens,
        block_comment,
        injection_regex,
        lsp_servers,
        roots,
    })
}

fn parse_languages_kdl(input: &str) -> Vec<LanguageDataRaw> {
    let doc: KdlDocument = input.parse().expect("failed to parse languages.kdl");
    doc.nodes()
        .iter()
        .filter(|n| n.name().value() == "language")
        .filter_map(parse_language_node)
        .collect()
}

fn kdl_value_to_json(value: &kdl::KdlValue) -> JsonValue {
    if let Some(s) = value.as_string() {
        JsonValue::String(s.to_string())
    } else if let Some(i) = value.as_integer() {
        JsonValue::Number((i as i64).into())
    } else if let Some(b) = value.as_bool() {
        JsonValue::Bool(b)
    } else {
        JsonValue::Null
    }
}

fn kdl_doc_to_json(doc: &KdlDocument) -> JsonValue {
    let mut map = serde_json::Map::new();
    for node in doc.nodes() {
        let key = node.name().value().to_string();
        if node.children().is_some() {
            map.insert(key, kdl_node_to_json(node));
        } else if let Some(entry) = node.entry(0) {
            map.insert(key, kdl_value_to_json(entry.value()));
        } else {
            map.insert(key, JsonValue::Bool(true));
        }
    }
    JsonValue::Object(map)
}

fn kdl_node_to_json(node: &KdlNode) -> JsonValue {
    let Some(children) = node.children() else {
        if let Some(entry) = node.entry(0) {
            return kdl_value_to_json(entry.value());
        }
        return JsonValue::Object(serde_json::Map::new());
    };
    kdl_doc_to_json(children)
}

fn parse_lsp_environment(children: Option<&KdlDocument>) -> HashMap<String, String> {
    let mut env = HashMap::new();
    let Some(env_node) = children.and_then(|c| c.get("environment")) else {
        return env;
    };

    for entry in env_node.entries() {
        if let Some(name) = entry.name()
            && let Some(value) = entry.value().as_string() {
                env.insert(name.value().to_string(), value.to_string());
            }
    }

    if let Some(env_children) = env_node.children() {
        for child in env_children.nodes() {
            if let Some(value) = child.entry(0).and_then(|e| e.value().as_string()) {
                env.insert(child.name().value().to_string(), value.to_string());
            }
        }
    }

    env
}

fn parse_lsp_server_node(node: &KdlNode) -> LspServerDefRaw {
    let name = node.name().value().to_string();
    let command = node
        .entry(0)
        .and_then(|e| {
            if e.name().is_none() {
                e.value().as_string().map(String::from)
            } else {
                None
            }
        })
        .unwrap_or_else(|| name.clone());

    let children = node.children();
    let args = parse_string_args(children, "args");
    let environment = parse_lsp_environment(children);
    let config_json = children
        .and_then(|c| c.get("config"))
        .map(|n| kdl_node_to_json(n).to_string());
    let source = children
        .and_then(|c| c.get("source"))
        .and_then(|n| n.entry(0))
        .and_then(|e| e.value().as_string())
        .map(String::from);
    let nix = children.and_then(|c| c.get("nix")).and_then(|n| {
        let entry = n.entry(0)?;
        if entry.value().as_bool() == Some(false) {
            None
        } else {
            entry.value().as_string().map(String::from)
        }
    });

    LspServerDefRaw {
        name,
        command,
        args,
        environment,
        config_json,
        source,
        nix,
    }
}

fn parse_lsp_kdl(input: &str) -> Vec<LspServerDefRaw> {
    let doc: KdlDocument = input.parse().expect("failed to parse lsp.kdl");
    doc.nodes().iter().map(parse_lsp_server_node).collect()
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let data_dir = PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .join("data/assets/language");

    let languages_path = data_dir.join("languages.kdl");
    println!("cargo:rerun-if-changed={}", languages_path.display());
    let languages_kdl = fs::read_to_string(&languages_path).expect("failed to read languages.kdl");
    let languages = parse_languages_kdl(&languages_kdl);
    let languages_bin = bincode::serialize(&languages).expect("failed to serialize languages");
    write_blob(&out_dir.join("languages.bin"), &languages_bin);

    let lsp_path = data_dir.join("lsp.kdl");
    println!("cargo:rerun-if-changed={}", lsp_path.display());
    let lsp_kdl = fs::read_to_string(&lsp_path).expect("failed to read lsp.kdl");
    let lsp_servers = parse_lsp_kdl(&lsp_kdl);
    let lsp_bin = bincode::serialize(&lsp_servers).expect("failed to serialize lsp");
    write_blob(&out_dir.join("lsp.bin"), &lsp_bin);
}
