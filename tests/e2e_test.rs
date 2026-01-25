mod e2e;

use e2e::TmuxHarness;
use std::time::Duration;

/// Path to the built binary
fn binary_path() -> String {
    // Use the debug build
    format!(
        "{}/target/debug/tuidaw",
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string())
    )
}

#[test]
fn test_displays_box_with_title() {
    let harness = TmuxHarness::new("box-title");
    harness.start(&binary_path()).expect("Failed to start app");

    // Wait a moment for rendering
    std::thread::sleep(Duration::from_millis(200));

    // Verify the box and title are displayed
    harness
        .assert_screen_contains("tuidaw")
        .expect("Should display 'tuidaw' title");

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
    let harness = TmuxHarness::new("quit");
    harness.start(&binary_path()).expect("Failed to start app");

    // Wait for app to start
    std::thread::sleep(Duration::from_millis(200));

    // Verify it's running
    assert!(harness.is_running(), "App should be running initially");

    // Send 'q' to quit
    harness.send_key("q").expect("Failed to send 'q'");

    // Wait for exit
    harness
        .wait_for_exit(Duration::from_secs(2))
        .expect("App should exit after pressing 'q'");

    assert!(!harness.is_running(), "App should have exited");
}
