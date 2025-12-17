use std::path::PathBuf;
use std::time::Duration;

use kitty_test_harness::{
    kitty_send_keys, pause_briefly, require_kitty, run_with_timeout,
    wait_for_screen_text_clean, with_kitty_capture,
};
use termwiz::input::{KeyCode, Modifiers};

const TEST_TIMEOUT: Duration = Duration::from_secs(15);

fn tome_cmd() -> String {
    env!("CARGO_BIN_EXE_tome").to_string()
}

fn tome_cmd_with_file_named(name: &str) -> String {
    format!("{} {}", tome_cmd(), name)
}

fn workspace_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn reset_test_file(name: &str) {
    let path = workspace_dir().join(name);
    let _ = std::fs::remove_file(&path);
}

#[serial_test::serial]
#[test]
fn terminal_can_be_toggled_and_input_commands() {
    if !require_kitty() {
        return;
    }

    let file = "kitty-test-terminal.txt";
    reset_test_file(file);
    run_with_timeout(TEST_TIMEOUT, || {
        with_kitty_capture(&workspace_dir(), &tome_cmd_with_file_named(file), |kitty| {
            // Allow the editor to boot and pre-warm the embedded shell.
            pause_briefly();
            pause_briefly();

            // Open terminal with Ctrl+t
            kitty_send_keys!(
                kitty,
                (KeyCode::Char('t'), Modifiers::CTRL)
            );
            
            // Wait for shell prompt or at least some output indicating terminal is open.
            // Since we don't know the exact prompt, we can try to run a command that produces known output.
            // "echo hello-terminal"
            
            pause_briefly(); // wait for PTY init
            
            kitty_send_keys!(
                kitty,
                KeyCode::Char('e'),
                KeyCode::Char('c'),
                KeyCode::Char('h'),
                KeyCode::Char('o'),
                KeyCode::Char(' '),
                KeyCode::Char('h'),
                KeyCode::Char('e'),
                KeyCode::Char('l'),
                KeyCode::Char('l'),
                KeyCode::Char('o'),
                KeyCode::Char('-'),
                KeyCode::Char('t'),
                KeyCode::Char('e'),
                KeyCode::Char('r'),
                KeyCode::Char('m'),
                KeyCode::Enter
            );

            let (_raw, clean) =
                wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
                    clean.contains("hello-term")
                });

            assert!(clean.contains("hello-term"), "Terminal should display echoed text. Screen: {:?}", clean);
            
            // Close terminal with Ctrl+t
            kitty_send_keys!(
                kitty,
                (KeyCode::Char('t'), Modifiers::CTRL)
            );
            
            // Should be back to editor view (which is empty/file)
            // We can't easily assert absence without waiting, but we can verify we can type into editor now.
            
            kitty_send_keys!(kitty, KeyCode::Char('i'));
            kitty_send_keys!(kitty, KeyCode::Char('e'), KeyCode::Char('d'), KeyCode::Char('i'), KeyCode::Char('t'));
            kitty_send_keys!(kitty, KeyCode::Escape);
            
            let (_raw, clean) =
                wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
                    clean.contains("edit")
                });
                
            assert!(clean.contains("edit"), "Should be able to edit buffer after closing terminal. Screen: {:?}", clean);
        });
    });
}
