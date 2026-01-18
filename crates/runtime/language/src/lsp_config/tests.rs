use super::*;

#[test]
fn parse_simple_server() {
	let kdl = r#"
rust-analyzer {
    source "https://github.com/rust-lang/rust-analyzer"
    nix rust-analyzer
}
"#;
	let servers = parse_lsp_configs(kdl).unwrap();
	assert_eq!(servers.len(), 1);

	let ra = &servers[0];
	assert_eq!(ra.name, "rust-analyzer");
	assert_eq!(ra.command, "rust-analyzer");
	assert!(ra.args.is_empty());
	assert_eq!(
		ra.source,
		Some("https://github.com/rust-lang/rust-analyzer".to_string())
	);
	assert_eq!(ra.nix, Some("rust-analyzer".to_string()));
}

#[test]
fn parse_server_with_different_command() {
	let kdl = r#"
angular ngserver {
    args --stdio --tsProbeLocations .
    source "https://github.com/angular/vscode-ng-language-service"
    nix angular-language-server
}
"#;
	let servers = parse_lsp_configs(kdl).unwrap();
	let angular = &servers[0];

	assert_eq!(angular.name, "angular");
	assert_eq!(angular.command, "ngserver");
	assert_eq!(angular.args, vec!["--stdio", "--tsProbeLocations", "."]);
	assert_eq!(angular.nix, Some("angular-language-server".to_string()));
}

#[test]
fn parse_server_with_config() {
	let kdl = r#"
typescript-language-server {
    args --stdio
    config {
        hostInfo helix
        typescript {
            inlayHints {
                enable #true
            }
        }
    }
}
"#;
	let servers = parse_lsp_configs(kdl).unwrap();
	let ts = &servers[0];

	assert!(ts.config.is_some());
	let config = ts.config.as_ref().unwrap();
	assert_eq!(config["hostInfo"], "helix");
	assert_eq!(config["typescript"]["inlayHints"]["enable"], true);
}

#[test]
fn parse_server_with_nix_false() {
	let kdl = r#"
my-server {
    nix #false
}
"#;
	let servers = parse_lsp_configs(kdl).unwrap();
	assert!(servers[0].nix.is_none());
}

#[test]
fn load_embedded_lsp_configs() {
	let servers = load_lsp_configs().expect("embedded lsp.kdl should parse");
	assert!(!servers.is_empty());

	// Check rust-analyzer exists
	let ra = servers
		.iter()
		.find(|s| s.name == "rust-analyzer")
		.expect("rust-analyzer should exist");
	assert_eq!(ra.command, "rust-analyzer");

	// Check typescript-language-server exists
	let ts = servers
		.iter()
		.find(|s| s.name == "typescript-language-server")
		.expect("typescript-language-server should exist");
	assert_eq!(ts.args, vec!["--stdio"]);
}

#[test]
fn parse_language_lsp_mapping_inline() {
	let kdl = r#"
language name=rust scope=source.rust {
    file-types rs
    roots Cargo.toml Cargo.lock
    language-servers rust-analyzer
}
language name=toml scope=source.toml {
    file-types toml
    language-servers taplo tombi
}
"#;
	let mapping = parse_language_lsp_mapping(kdl).unwrap();

	let rust = mapping.get("rust").unwrap();
	assert_eq!(rust.servers, vec!["rust-analyzer"]);
	assert_eq!(rust.roots, vec!["Cargo.toml", "Cargo.lock"]);

	let toml = mapping.get("toml").unwrap();
	assert_eq!(toml.servers, vec!["taplo", "tombi"]);
	assert!(toml.roots.is_empty());
}

#[test]
fn load_embedded_language_lsp_mapping() {
	let mapping = load_language_lsp_mapping().expect("embedded languages.kdl should parse");
	assert!(!mapping.is_empty());

	// Check rust has rust-analyzer
	let rust = mapping.get("rust").expect("rust should have servers");
	assert!(rust.servers.contains(&"rust-analyzer".to_string()));
	assert!(rust.roots.contains(&"Cargo.toml".to_string()));

	// Check python has servers
	let python = mapping.get("python").expect("python should have servers");
	assert!(!python.servers.is_empty());
}
