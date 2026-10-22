use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn per_url_export_errors_return_an_xargs_continue_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_chatbot-exporter"))
        .arg("https://example.com/share/unknown")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("No conversation exporter is registered")
    );
}

#[test]
fn invalid_configuration_returns_xargs_stop_status() {
    let output = Command::new(env!("CARGO_BIN_EXE_chatbot-exporter"))
        .args(["--timeout-secs", "0"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(255));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--timeout-secs requires a positive integer")
    );
}

#[test]
fn stdin_item_errors_continue_and_return_ordinary_failure() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_chatbot-exporter"))
        .arg("--stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"\n# skipped\nnot-a-url\nhttps://example.com/share/no-provider\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let errors = String::from_utf8(output.stderr).unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(errors.contains("Line 3: Provide a valid conversation share URL."));
    assert!(errors.contains("Line 4: No conversation exporter is registered"));
    assert!(errors.contains("Failed to export 2 URL(s)"));
}
