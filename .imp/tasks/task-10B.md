# Task 10B: LSP UI Integration Tests with Kitty Harness

## Model Directive

This document specifies comprehensive integration tests for the LSP UI features implemented in Task 10A. Tests use the `kitty-test-harness` to verify actual terminal rendering and user workflows with real language servers provided via Nix.

**Context**: Task 10A implemented popup infrastructure, diagnostics display, hover tooltips, completion menus, signature help, code actions, diagnostic panels, navigation features, and inlay hints. These features need real-world validation beyond unit tests.

**Scope**: Create user-story-driven integration tests that launch xeno in a real kitty terminal, interact with actual language servers (rust-analyzer), and verify the UI renders correctly.

---

## Critical Implementation Note

<mandatory_execution_requirements>

**AGENTS MUST FILL FUNCTIONALITY GAPS**

These tests verify end-to-end user workflows. If a test requires functionality that doesn't exist or is broken, **you must implement/fix that functionality as part of this task**.

Examples of gaps that must be filled:
- If the diagnostics panel doesn't populate from LSP data, implement the data flow
- If hover popup doesn't render markdown correctly, fix the rendering
- If completion acceptance doesn't insert text, fix the insertion logic
- If the LSP client doesn't start automatically for Rust files, wire it up

**Do NOT:**
- Skip tests because "the feature isn't working yet"
- Write tests that pass by checking nothing
- Leave TODOs for "someone else" to fix

**DO:**
- Investigate why a feature isn't working
- Implement missing glue code
- Fix bugs discovered during test development
- Document any architectural issues in AGENTS.md

</mandatory_execution_requirements>

---

## Test Infrastructure

### Environment Variable

Tests are gated by `LSP_TESTS=1` environment variable, similar to existing patterns:

```bash
LSP_TESTS=1 KITTY_TESTS=1 DISPLAY=:0 nix develop -c cargo test -p xeno-term --test lsp_hover -- --nocapture --test-threads=1
```

Both `LSP_TESTS=1` AND `KITTY_TESTS=1` are required (LSP tests use kitty harness).

### Nix Dependencies

**Location**: `crates/term/tests/lsp-deps.nix` (co-located with tests, NOT in root nix folder)

This file provides language servers for tests:

```nix
# Language server dependencies for LSP integration tests.
# Used by LSP_TESTS=1 to provide language servers via nix-shell.
{
  pkgs ? import <nixpkgs> { },
}:

pkgs.mkShell {
  packages = [
    pkgs.rust-analyzer  # Rust LSP - primary test target
    pkgs.rustc          # Rust compiler (needed by rust-analyzer)
    pkgs.cargo          # Cargo (needed by rust-analyzer)
  ];
}
```

### Test Fixtures

**Location**: `crates/term/tests/fixtures/lsp/`

Pre-created Rust projects with known content that produces predictable LSP responses:

```
crates/term/tests/fixtures/lsp/
├── rust-basic/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs      # Has: unused var, type error, documented fn
├── rust-completion/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs       # Has: struct with fields for completion testing
└── rust-navigation/
    ├── Cargo.toml
    └── src/
        ├── lib.rs       # Has: function definitions
        └── other.rs     # Has: references to lib.rs functions
```

---

## Test File Structure

All LSP integration tests go in `crates/term/tests/`:

| File | Purpose |
|------|---------|
| `lsp_diagnostics.rs` | Diagnostic display, gutter signs, navigation |
| `lsp_hover.rs` | Hover popup rendering |
| `lsp_completion.rs` | Completion menu, filtering, acceptance |
| `lsp_navigation.rs` | Go to definition, find references |
| `lsp_code_actions.rs` | Code actions popup, quickfix application |
| `lsp_helpers.rs` | Shared utilities for LSP tests |

---

## Implementation Roadmap

### Phase 1: Test Infrastructure Setup

**Files**:
- `crates/term/tests/lsp-deps.nix` (new)
- `crates/term/tests/lsp_helpers.rs` (new)
- `crates/term/tests/fixtures/lsp/rust-basic/` (new)

- [x] 1.1 Create `lsp-deps.nix` with rust-analyzer
  ```nix
  { pkgs ? import <nixpkgs> { } }:
  pkgs.mkShell {
    packages = [
      pkgs.rust-analyzer
      pkgs.rustc
      pkgs.cargo
    ];
  }
  ```

- [x] 1.2 Create `lsp_helpers.rs` with shared utilities
  ```rust
  //! Shared utilities for LSP integration tests.

  use std::path::PathBuf;
  use std::time::Duration;
  use kitty_test_harness::KittyHarness;

  /// Check if LSP tests should run.
  pub fn require_lsp_tests() -> bool {
      if std::env::var("LSP_TESTS").is_err() {
          eprintln!("Skipping LSP test (set LSP_TESTS=1 to run)");
          return false;
      }
      kitty_test_harness::require_kitty()
  }

  /// Returns path to LSP test fixtures.
  pub fn fixtures_dir() -> PathBuf {
      PathBuf::from(env!("CARGO_MANIFEST_DIR"))
          .join("tests/fixtures/lsp")
  }

  /// Wait for LSP to initialize (diagnostics to appear).
  /// rust-analyzer needs time to index the project.
  pub fn wait_for_lsp_ready(kitty: &KittyHarness, timeout: Duration) {
      // Wait for status bar to show LSP ready, or diagnostics to appear
      // This is a key integration point - may need to add LSP status indicator
  }

  /// Helper to type in insert mode and return to normal.
  pub fn type_and_escape(kitty: &KittyHarness, text: &str) { ... }
  ```

- [x] 1.3 Create `rust-basic` fixture project
  ```rust
  // fixtures/lsp/rust-basic/src/main.rs

  /// A well-documented function for hover testing.
  ///
  /// # Arguments
  /// * `x` - The input value
  ///
  /// # Returns
  /// The input plus one
  fn documented_function(x: i32) -> i32 {
      x + 1
  }

  fn main() {
      // Unused variable - should produce warning diagnostic
      let unused_var = 42;

      // Type error - should produce error diagnostic
      let type_error: String = 123;

      // Call documented function
      let result = documented_function(5);
      println!("{}", result);
  }
  ```

- [x] 1.4 Create `Cargo.toml` for fixture
  ```toml
  [package]
  name = "rust-basic"
  version = "0.1.0"
  edition = "2021"
  ```

- [x] 1.5 Verify: `cargo build --workspace`

**CHECKPOINT 1**: Test infrastructure exists, fixtures created

---

### Phase 2: Diagnostics Display Tests

**User Stories**:
1. "As a user, when I open a Rust file with errors, I see red markers in the gutter"
2. "As a user, I see squiggly underlines under the error locations"
3. "As a user, I can press `]d` to jump to the next diagnostic"

**Files**:
- `crates/term/tests/lsp_diagnostics.rs` (new)

- [x] 2.1 Test: `diagnostics_show_gutter_signs`
  ```rust
  /// Opening a file with errors shows diagnostic signs in gutter.
  #[serial_test::serial]
  #[test]
  fn diagnostics_show_gutter_signs() {
      if !require_lsp_tests() { return; }

      run_with_timeout(Duration::from_secs(30), || {
          let fixture = fixtures_dir().join("rust-basic");
          with_kitty_capture(&fixture, &xeno_cmd_with_file("src/main.rs"), |kitty| {
              // Wait for LSP to initialize and publish diagnostics
              wait_for_lsp_ready(kitty, Duration::from_secs(15));

              // Verify gutter shows diagnostic sign (E for error, W for warning)
              let (_raw, clean) = wait_for_screen_text_clean(
                  kitty, Duration::from_secs(5),
                  |_r, clean| clean.contains("E") || clean.contains("W")
              );

              // The gutter should show error/warning indicators
              assert!(
                  clean.lines().any(|line| /* check gutter column */),
                  "Gutter should show diagnostic signs"
              );
          });
      });
  }
  ```

- [x] 2.2 Test: `diagnostics_navigation_next_prev`
  ```rust
  /// Pressing ]d jumps to next diagnostic, [d to previous.
  #[serial_test::serial]
  #[test]
  fn diagnostics_navigation_next_prev() {
      if !require_lsp_tests() { return; }

      run_with_timeout(Duration::from_secs(30), || {
          let fixture = fixtures_dir().join("rust-basic");
          with_kitty_capture(&fixture, &xeno_cmd_with_file("src/main.rs"), |kitty| {
              wait_for_lsp_ready(kitty, Duration::from_secs(15));

              // Move to top of file
              kitty_send_keys!(kitty, KeyCode::Char('g'), KeyCode::Char('g'));
              pause_briefly();

              // Jump to next diagnostic
              kitty_send_keys!(kitty, KeyCode::Char(']'), KeyCode::Char('d'));

              // Verify cursor moved to diagnostic line
              // Should show notification with diagnostic message
              let (_raw, clean) = wait_for_screen_text_clean(
                  kitty, Duration::from_secs(3),
                  |_r, clean| clean.contains("unused") || clean.contains("error")
              );

              assert!(
                  clean.contains("unused") || clean.contains("mismatched"),
                  "Should show diagnostic message in notification"
              );
          });
      });
  }
  ```

- [x] 2.3 Test: `diagnostics_underline_rendering`
  - Verify underline styles appear under error spans
  - May need to extract color/style info from raw terminal output

- [x] 2.4 **GAP CHECK**: If diagnostics don't appear, investigate and fix:
  - Is `DocumentStateManager` receiving `publishDiagnostics`?
  - Is `prepare_diagnostics()` being called during render?
  - Is the gutter `diagnostic_severity` being set?
  - **FIX APPLIED**: The issue was a type mismatch - render code was looking for
    `Arc<xeno_api::lsp::LspManager>` but extension inserted `Arc<extension::LspManager>`.
    Fixed by having the extension insert `Arc<Registry>` which the render code now
    uses directly to fetch diagnostics from `ClientState` via `client.diagnostics(&uri)`.

- [x] 2.5 Verify: `LSP_TESTS=1 KITTY_TESTS=1 cargo test -p xeno-term --test lsp_diagnostics`
  - **FIX APPLIED**: Diagnostics now flow correctly:
    LSP Server -> ClientState.diagnostics -> Registry.get() -> client.diagnostics() -> gutter rendering

**CHECKPOINT 2**: Diagnostics visible and navigable in real terminal

---

### Phase 3: Hover Popup Tests

**User Stories**:
1. "As a user, pressing K on a function shows its documentation"
2. "As a user, the hover popup dismisses when I press any key"
3. "As a user, I see type information for variables"

**Files**:
- `crates/term/tests/lsp_hover.rs` (new)

- [x] 3.1 Test: `hover_shows_documentation`
  ```rust
  /// Pressing K on a documented function shows hover popup with docs.
  #[serial_test::serial]
  #[test]
  fn hover_shows_documentation() {
      if !require_lsp_tests() { return; }

      run_with_timeout(Duration::from_secs(30), || {
          let fixture = fixtures_dir().join("rust-basic");
          with_kitty_capture(&fixture, &xeno_cmd_with_file("src/main.rs"), |kitty| {
              wait_for_lsp_ready(kitty, Duration::from_secs(15));

              // Navigate to the documented_function call
              kitty_send_keys!(kitty, KeyCode::Char('/'));
              type_chars(kitty, "documented_function");
              kitty_send_keys!(kitty, KeyCode::Enter);
              kitty_send_keys!(kitty, KeyCode::Escape);
              pause_briefly();

              // Trigger hover
              kitty_send_keys!(kitty, (KeyCode::Char('K'), Modifiers::SHIFT));

              // Verify hover popup appears with documentation
              let (_raw, clean) = wait_for_screen_text_clean(
                  kitty, Duration::from_secs(5),
                  |_r, clean| clean.contains("well-documented") || clean.contains("Arguments")
              );

              assert!(
                  clean.contains("documented") || clean.contains("i32"),
                  "Hover should show function documentation. Got: {clean}"
              );
          });
      });
  }
  ```

- [x] 3.2 Test: `hover_dismisses_on_keypress`
  ```rust
  /// Hover popup dismisses when any key is pressed.
  #[serial_test::serial]
  #[test]
  fn hover_dismisses_on_keypress() {
      // Show hover, press Escape, verify popup gone
  }
  ```

- [x] 3.3 Test: `hover_shows_type_info`
  - Hover on variable shows its type

- [x] 3.4 **GAP CHECK**: If hover doesn't work:
  - Is `Editor::show_hover()` being called?
  - Is `LspManager::hover()` returning data?
  - Is `HoverPopup` rendering correctly?
  - Is cursor screen position calculated correctly?
  - **RESULT**: All tests pass - hover functionality works correctly with rust-analyzer.

- [x] 3.5 Verify: `LSP_TESTS=1 KITTY_TESTS=1 cargo test -p xeno-term --test lsp_hover`

**CHECKPOINT 3**: Hover popup shows documentation from rust-analyzer

---

### Phase 4: Completion Menu Tests

**User Stories**:
1. "As a user, pressing Ctrl+Space shows completion menu"
2. "As a user, typing filters the completion list"
3. "As a user, pressing Tab inserts the selected completion"
4. "As a user, completions appear automatically after `.`"

**Files**:
- `crates/term/tests/lsp_completion.rs` (new)
- `crates/term/tests/fixtures/lsp/rust-completion/` (new)

- [x] 4.1 Create `rust-completion` fixture
  ```rust
  // fixtures/lsp/rust-completion/src/lib.rs

  pub struct Config {
      pub name: String,
      pub value: i32,
      pub enabled: bool,
  }

  impl Config {
      pub fn new() -> Self {
          Self {
              name: String::new(),
              value: 0,
              enabled: false,
          }
      }

      pub fn with_name(mut self, name: &str) -> Self {
          self.name = name.to_string();
          self
      }
  }

  fn test_completion() {
      let config = Config::new();
      // Cursor here for testing: config.
  }
  ```

- [x] 4.2 Test: `completion_manual_trigger`
  ```rust
  /// Ctrl+Space triggers completion menu.
  #[serial_test::serial]
  #[test]
  fn completion_manual_trigger() {
      if !require_lsp_tests() { return; }

      run_with_timeout(Duration::from_secs(30), || {
          let fixture = fixtures_dir().join("rust-completion");
          with_kitty_capture(&fixture, &xeno_cmd_with_file("src/lib.rs"), |kitty| {
              wait_for_lsp_ready(kitty, Duration::from_secs(15));

              // Go to end of file, enter insert mode
              kitty_send_keys!(kitty, KeyCode::Char('G'));
              kitty_send_keys!(kitty, KeyCode::Char('o'));
              type_chars(kitty, "let c = Config::new();");
              kitty_send_keys!(kitty, KeyCode::Enter);
              type_chars(kitty, "c.");

              // Trigger completion
              kitty_send_keys!(kitty, (KeyCode::Char(' '), Modifiers::CTRL));

              // Verify completion menu shows struct fields
              let (_raw, clean) = wait_for_screen_text_clean(
                  kitty, Duration::from_secs(5),
                  |_r, clean| clean.contains("name") && clean.contains("value")
              );

              assert!(clean.contains("name"), "Should show 'name' field");
              assert!(clean.contains("value"), "Should show 'value' field");
              assert!(clean.contains("enabled"), "Should show 'enabled' field");
          });
      });
  }
  ```

- [x] 4.3 Test: `completion_filtering`
  ```rust
  /// Typing filters the completion list.
  #[serial_test::serial]
  #[test]
  fn completion_filtering() {
      // Open completion, type "na", verify only "name" remains
  }
  ```

- [x] 4.4 Test: `completion_acceptance`
  ```rust
  /// Tab accepts the selected completion.
  #[serial_test::serial]
  #[test]
  fn completion_acceptance() {
      // Open completion, select item, Tab, verify text inserted
  }
  ```

- [x] 4.5 Test: `completion_auto_trigger_dot`
  ```rust
  /// Completion appears automatically after typing '.'.
  #[serial_test::serial]
  #[test]
  fn completion_auto_trigger_dot() {
      // Type "config." and verify menu appears without Ctrl+Space
  }
  ```

- [x] 4.6 **GAP CHECK**: If completion doesn't work:
  - Is `trigger_completion()` sending LSP request?
  - Is completion response being parsed correctly?
  - Is `CompletionPopup` being shown?
  - Is Tab handler calling `try_accept_completion()`?
  - **RESULT**: All completion tests pass - manual trigger, filtering, acceptance, and auto-trigger after dot all work correctly with rust-analyzer.

- [x] 4.7 Verify: `LSP_TESTS=1 KITTY_TESTS=1 cargo test -p xeno-term --test lsp_completion`

**CHECKPOINT 4**: Completion menu works end-to-end with rust-analyzer

---

### Phase 5: Navigation Tests

**User Stories**:
1. "As a user, pressing `gd` on a function call jumps to its definition"
2. "As a user, pressing `gr` shows all references to the symbol"
3. "As a user, if there are multiple definitions, I see a picker"

**Files**:
- `crates/term/tests/lsp_navigation.rs` (new)
- `crates/term/tests/fixtures/lsp/rust-navigation/` (new)

- [x] 5.1 Create `rust-navigation` fixture
  ```rust
  // fixtures/lsp/rust-navigation/src/lib.rs
  pub fn shared_function() -> i32 {
      42
  }

  // fixtures/lsp/rust-navigation/src/other.rs
  use crate::shared_function;

  pub fn caller_one() -> i32 {
      shared_function()  // Reference 1
  }

  pub fn caller_two() -> i32 {
      shared_function()  // Reference 2
  }
  ```

- [x] 5.2 Test: `goto_definition_jumps`
  ```rust
  /// gd on function call jumps to definition.
  #[serial_test::serial]
  #[test]
  fn goto_definition_jumps() {
      if !require_lsp_tests() { return; }

      run_with_timeout(Duration::from_secs(30), || {
          let fixture = fixtures_dir().join("rust-navigation");
          with_kitty_capture(&fixture, &xeno_cmd_with_file("src/other.rs"), |kitty| {
              wait_for_lsp_ready(kitty, Duration::from_secs(15));

              // Find shared_function call
              kitty_send_keys!(kitty, KeyCode::Char('/'));
              type_chars(kitty, "shared_function");
              kitty_send_keys!(kitty, KeyCode::Enter);
              kitty_send_keys!(kitty, KeyCode::Escape);

              // Go to definition
              kitty_send_keys!(kitty, KeyCode::Char('g'), KeyCode::Char('d'));

              // Verify we jumped to lib.rs (status bar shows filename)
              let (_raw, clean) = wait_for_screen_text_clean(
                  kitty, Duration::from_secs(5),
                  |_r, clean| clean.contains("lib.rs")
              );

              assert!(clean.contains("lib.rs"), "Should jump to lib.rs");
              assert!(clean.contains("pub fn shared_function"), "Should show definition");
          });
      });
  }
  ```

- [x] 5.3 Test: `find_references_shows_list`
  ```rust
  /// gr shows references panel with all usages.
  #[serial_test::serial]
  #[test]
  fn find_references_shows_list() {
      // Navigate to shared_function definition
      // Press gr
      // Verify references panel shows both callers
  }
  ```

- [x] 5.4 Test: `references_panel_navigation`
  ```rust
  /// Enter on reference in panel jumps to that location.
  ```

- [ ] 5.5 **GAP CHECK**: If navigation doesn't work:
  - Is `goto_definition()` opening the target file?
  - Is `find_references()` populating the panel?
  - Is the references panel wired to the dock system?
  - **GAP FOUND**: `gd` (goto_definition_jumps, goto_definition_from_import) tests timeout
  - Tests `find_references_shows_list` and `references_panel_navigation` pass

- [ ] 5.6 Verify: `LSP_TESTS=1 KITTY_TESTS=1 cargo test -p xeno-term --test lsp_navigation`
  - PARTIAL: 2/4 tests pass (references work, goto_definition times out)

**CHECKPOINT 5**: Navigation features work with real LSP

---

### Phase 6: Code Actions Tests

**User Stories**:
1. "As a user, I see a lightbulb when code actions are available"
2. "As a user, pressing `ga` (or `<space>a`) shows available actions"
3. "As a user, selecting a quickfix applies the change"

**Files**:
- `crates/term/tests/lsp_code_actions.rs` (new)

- [ ] 6.1 Test: `code_actions_quickfix`
  ```rust
  /// Code action quickfix removes unused import.
  #[serial_test::serial]
  #[test]
  fn code_actions_quickfix() {
      if !require_lsp_tests() { return; }

      run_with_timeout(Duration::from_secs(30), || {
          // Create temp file with unused import
          let fixture = fixtures_dir().join("rust-basic");
          with_kitty_capture(&fixture, &xeno_cmd_with_file("src/main.rs"), |kitty| {
              wait_for_lsp_ready(kitty, Duration::from_secs(15));

              // Navigate to line with unused variable
              kitty_send_keys!(kitty, KeyCode::Char('/'));
              type_chars(kitty, "unused_var");
              kitty_send_keys!(kitty, KeyCode::Enter);
              kitty_send_keys!(kitty, KeyCode::Escape);

              // Show code actions (space + a, based on keybinding)
              kitty_send_keys!(kitty, KeyCode::Char(' '));
              kitty_send_keys!(kitty, KeyCode::Char('a'));

              // Verify code actions popup appears
              let (_raw, clean) = wait_for_screen_text_clean(
                  kitty, Duration::from_secs(5),
                  |_r, clean| clean.contains("Remove") || clean.contains("prefix")
              );

              assert!(
                  clean.contains("Remove") || clean.contains("_"),
                  "Should show quickfix to remove/prefix unused variable"
              );
          });
      });
  }
  ```

- [ ] 6.2 Test: `code_actions_lightbulb`
  - Verify lightbulb appears in gutter on lines with actions

- [ ] 6.3 **GAP CHECK**: If code actions don't work:
  - Is `show_code_actions()` requesting from LSP?
  - Is `CodeActionsPopup` rendering?
  - Is workspace edit application working?

- [ ] 6.4 Verify: `LSP_TESTS=1 KITTY_TESTS=1 cargo test -p xeno-term --test lsp_code_actions`

**CHECKPOINT 6**: Code actions work end-to-end

---

## Test Execution

### Running All LSP Tests

```bash
# Full test suite
LSP_TESTS=1 KITTY_TESTS=1 DISPLAY=:0 nix develop -c cargo test -p xeno-term --test 'lsp_*' -- --nocapture --test-threads=1

# Individual test file
LSP_TESTS=1 KITTY_TESTS=1 DISPLAY=:0 nix develop -c cargo test -p xeno-term --test lsp_hover -- --nocapture --test-threads=1

# Specific test
LSP_TESTS=1 KITTY_TESTS=1 DISPLAY=:0 nix develop -c cargo test -p xeno-term --test lsp_hover hover_shows_documentation -- --nocapture --test-threads=1
```

- Note that the Kitty test harness has been proven to work in your environment, your terminal shell environment CAN spawn a wayland window on the user's machine.
- There is no 'flaky envionment' excuse. Well-tested LSPs are also known to work fine in this environment. The only explanation for failures is the xeno code iteself.

---

## Success Criteria

- [ ] All test files created and passing
- [ ] Tests run against real rust-analyzer, not mocks
- [ ] Tests verify actual terminal rendering via kitty-test-harness
- [ ] Any functionality gaps discovered during testing are fixed
- [ ] Tests are reproducible (fixtures produce consistent LSP responses)
- [ ] Tests complete in reasonable time (<30s each)
- [ ] No flaky tests (proper waits for LSP initialization)

---

## Files Summary

| File | Type | Phase |
|------|------|-------|
| `crates/term/tests/lsp-deps.nix` | New | 1 |
| `crates/term/tests/lsp_helpers.rs` | New | 1 |
| `crates/term/tests/fixtures/lsp/rust-basic/` | New | 1 |
| `crates/term/tests/fixtures/lsp/rust-completion/` | New | 4 |
| `crates/term/tests/fixtures/lsp/rust-navigation/` | New | 5 |
| `crates/term/tests/lsp_diagnostics.rs` | New | 2 |
| `crates/term/tests/lsp_hover.rs` | New | 3 |
| `crates/term/tests/lsp_completion.rs` | New | 4 |
| `crates/term/tests/lsp_navigation.rs` | New | 5 |
| `crates/term/tests/lsp_code_actions.rs` | New | 6 |

---

## Debugging Tips

### LSP Not Initializing

1. Check rust-analyzer is in PATH: `which rust-analyzer`
2. Check fixture has valid `Cargo.toml`
3. Increase timeout for `wait_for_lsp_ready()`
4. Add logging: `XENO_TEST_LOG=/tmp/xeno.log`

### Test Timing Out

1. LSP initialization can take 10-15s for first index
2. Use `pause_briefly()` between actions
3. Check if LSP process is actually running

### Screen Content Not Matching

1. Print `clean` output in assertions for debugging
2. Use `extract_row_colors_parsed()` for style verification
3. Check terminal size assumptions

### Unix Socket Path Too Long

If you see "Errors parsing configuration" with "Invalid listen_on=unix:/.../kitty-test-*.sock", the socket path exceeds the ~108 character limit on Linux.

**Workaround**: Don't run xeno from the fixture directory. Instead:
- Run `with_kitty_capture()` from `workspace_dir()` (the crate root)
- Pass the full absolute path to the fixture file

```rust
// WRONG - socket path will be too long
with_kitty_capture(&fixtures_dir().join("rust-basic"), &xeno_cmd_with_file("src/main.rs"), |kitty| { ... });

// CORRECT - run from crate root, use absolute file path
let fixture_file = fixtures_dir().join("rust-basic/src/main.rs");
with_kitty_capture(&workspace_dir(), &xeno_cmd_with_file(&fixture_file.display().to_string()), |kitty| { ... });
```

### Fixture Changes Not Reflected

1. rust-analyzer caches aggressively
2. Delete `target/` in fixtures between runs
3. Touch files to invalidate cache
