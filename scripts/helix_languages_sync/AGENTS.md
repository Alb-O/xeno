# Helix Languages Sync

This playbook defines the manual sync process for upstream Helix `languages.toml` into Xeno's NUON registry assets.

Xeno intentionally does not do a 1:1 machine translation because the schemas diverge. We still want a fast, repeatable way to pick up upstream changes.

## Source Of Truth

* Upstream file: `helix-editor/helix` `languages.toml` (repo root)
* Xeno targets:
  * `crates/registry/src/domains/languages/assets/languages.nuon`
  * `crates/registry/src/domains/lsp_servers/assets/lsp_servers.nuon`
  * `crates/registry/src/domains/grammars/assets/grammars.nuon` (when grammar entries change)
* Sync baseline metadata:
  * in-script defaults in `scripts/helix_languages_sync/report.sh` (`HELIX_UPSTREAM`, `DEFAULT_HEAD_REF`, `DEFAULT_BASE_REF`)

## Field Mapping

Use this mapping when translating changes:

* `[[language]] name` -> `languages.nuon` `common.name`
* `scope` -> `scope`
* `grammar` -> `grammar_name` (and ensure matching grammar exists in `grammars.nuon`)
* `injection-regex` -> `injection_regex`
* `file-types`:
  * extensions (`"rs"`) -> `extensions`
  * exact filenames/globs (`{ glob = ... }`) -> `filenames` or `globs`
* `shebangs` -> `shebangs`
* `roots` -> `roots`
* `comment-token` / `comment-tokens` -> `comment_tokens`
* `block-comment-tokens` -> `block_comment`
* `language-servers` -> `lsp_servers`
* `auto-format` -> `auto_format`
* `[language-server.<name>]`:
  * `command` -> `lsp_servers.nuon` `command`
  * `args` -> `args`
  * `environment` -> `environment`
  * `config` -> `config_json` (serialized JSON string)

Helix-only fields with no current Xeno target should be skipped unless you are explicitly extending Xeno schema in the same change.

## Review Workflow

1. Generate the upstream change report:

```bash
./scripts/helix_languages_sync/report.sh
# optional explicit range
./scripts/helix_languages_sync/report.sh <base_ref> <head_ref>
```

2. Read the generated report and patch paths from script output.
3. Apply representable changes to the NUON assets listed above.
4. Validate:

```bash
cargo check -p xeno-registry
cargo test -p xeno-language test_queries_from_registry
```

5. After merge, update `scripts/helix_languages_sync/report.sh`:
  * set `DEFAULT_BASE_REF` to the report head commit you just reviewed

## Recap

* sync Helix languages.toml changes into Xeno NUON assets
* use this playbook and run `scripts/helix_languages_sync/report.sh`
* apply only representable changes to:
  * `crates/registry/src/domains/languages/assets/languages.nuon`
  * `crates/registry/src/domains/lsp_servers/assets/lsp_servers.nuon`
  * `crates/registry/src/domains/grammars/assets/grammars.nuon`
* do not auto-translate unsupported helix-only fields
* run validation:
  * `cargo check -p xeno-registry`
  * `cargo test -p xeno-language test_queries_from_registry`
* update `DEFAULT_BASE_REF` in `scripts/helix_languages_sync/report.sh` to the reviewed head commit
