mod e2e;

use e2e::TmuxHarness;
use std::time::Duration;

/// Path to the built binary
fn binary_path() -> String {
    // Use the debug build
    let path = format!(
        "{}/target/debug/ilex",
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string())
    );
    assert!(
        std::path::Path::new(&path).exists(),
        "Binary not found at {}. Run `cargo build` first.",
        path
    );
    path
}

/// Check if tmux is available, skip test if not
fn require_tmux() -> bool {
    std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn test_displays_box_with_title() {
    if !require_tmux() {
        eprintln!("tmux not found, skipping test");
        return;
    }

    let harness = TmuxHarness::new("box-title");
    harness.start(&binary_path()).expect("Failed to start app");

    // Wait a moment for rendering
    std::thread::sleep(Duration::from_millis(200));

    // App starts with no instruments → AddPane is shown
    harness
        .assert_screen_contains("Add Instrument")
        .expect("Should display 'Add Instrument' dialog");

    // Verify the frame header renders
    harness
        .assert_screen_contains("ILEX")
        .expect("Should display 'ILEX' frame header");

    // Verify box borders are present (corner characters)
    let screen = harness.capture_screen().expect("Should capture screen");
    assert!(
        screen.contains("┌") || screen.contains("+") || screen.contains("╭"),
        "Should display box border (top-left corner)\nScreen:\n{}",
        screen
    );
}

#[test]
fn test_quit_with_q() {
    if !require_tmux() {
        eprintln!("tmux not found, skipping test");
        return;
    }

    let harness = TmuxHarness::new("quit");
    harness.start(&binary_path()).expect("Failed to start app");

    // Wait for app to start
    std::thread::sleep(Duration::from_millis(200));

    // Verify it's running
    assert!(harness.is_running(), "App should be running initially");

    // Send Ctrl+q to quit (global quit binding, works from any pane)
    harness.send_key("C-q").expect("Failed to send 'C-q'");

    // Wait for exit
    harness
        .wait_for_exit(Duration::from_secs(3))
        .expect("App should exit after pressing Ctrl+q");

    assert!(!harness.is_running(), "App should have exited");
}
