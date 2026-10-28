#![cfg(feature = "cli")]

use std::process::Command;

#[test]
fn standalone_help_does_not_require_a_configuration() {
    for flag in ["-h", "--help"] {
        let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
            .arg(flag)
            .output()
            .unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("<command> <interval-seconds> [key-delay-seconds]"));
        assert!(!stdout.contains("default: 60"));
    }
}

#[test]
fn valid_full_arguments_with_help_skip_environment_checks() {
    let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
        .args(["--help", "window", "Minecraft", "hello", "60"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
}

#[test]
fn invalid_full_arguments_with_help_return_errors() {
    for (args, message) in [
        (
            vec!["--help", "window", "Minecraft", "!UNKNOWN!", "60"],
            "unsupported input token",
        ),
        (
            vec!["--help", "screen", "worker", "status", "later"],
            "interval",
        ),
        (
            vec!["--help", "unknown", "target", "hello"],
            "unsupported target",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
    }
}

#[test]
fn argument_errors_include_usage_once() {
    for (args, message) in [
        (
            vec!["window", "Minecraft"],
            "missing required positional arguments",
        ),
        (
            vec!["window", "Minecraft", "hello"],
            "missing required interval-seconds",
        ),
        (
            vec!["--background", "window", "Minecraft", "hello"],
            "missing required interval-seconds",
        ),
        (
            vec!["--help", "window", "Minecraft", "hello"],
            "missing required interval-seconds",
        ),
        (vec!["unknown", "target", "hello"], "unsupported target"),
        (
            vec!["window", "Minecraft", "!UNKNOWN!", "60"],
            "unsupported input token",
        ),
        (vec!["screen", "worker", "status", "later"], "interval"),
        (
            vec!["window", "Minecraft", "hello", "60", "bad"],
            "key delay",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("Application error: "));
        assert!(stderr.contains(message), "missing diagnostic in: {stderr}");
        assert_eq!(stderr.matches("Usage:").count(), 1, "{stderr}");
    }
}

#[test]
fn positional_delay_is_available_for_screen() {
    let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
        .args(["--help", "screen", "worker", "hello", "60", "0.12"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
}

#[test]
fn end_of_options_marker_is_rejected() {
    for args in [
        &["--", "window", "Editor", "hello"][..],
        &["--help", "--", "window", "Editor", "hello"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("unsupported option: --"), "{stderr}");
        assert_eq!(stderr.matches("Usage:").count(), 1, "{stderr}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn invalid_configuration_is_reported_before_environment() {
    let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
        .args(["window", "Minecraft", "!UNKNOWN!", "60"])
        .env("XDG_SESSION_TYPE", "wayland")
        .env("WAYLAND_DISPLAY", "input-pulse-test")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("unsupported input token"));
    assert!(!error.contains("does not support Wayland sessions"));
}

#[cfg(target_os = "linux")]
#[test]
fn environment_error_does_not_include_usage() {
    for (session_type, wayland_display) in [
        (Some("wayland"), None),
        (None, Some("input-pulse-test")),
        (Some("wayland"), Some("input-pulse-test")),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_input-pulse"));
        command.args(["window", "Minecraft", "hello", "60"]);
        match session_type {
            Some(value) => {
                command.env("XDG_SESSION_TYPE", value);
            }
            None => {
                command.env_remove("XDG_SESSION_TYPE");
            }
        }
        match wayland_display {
            Some(value) => {
                command.env("WAYLAND_DISPLAY", value);
            }
            None => {
                command.env_remove("WAYLAND_DISPLAY");
            }
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("Application error: "));
        assert!(stderr.contains("does not support Wayland sessions"));
        assert!(!stderr.contains("Usage:"), "{stderr}");
        assert!(output.stdout.is_empty());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn ssh_without_session_type_reports_missing_or_invalid_display() {
    for display in [
        None,
        Some(""),
        Some("invalid-display"),
        Some("127.0.0.1:65535"),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_input-pulse"));
        command
            .args(["window", "Minecraft", "hello", "60"])
            .env_remove("XDG_SESSION_TYPE")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("XAUTHORITY");
        match display {
            Some(value) => {
                command.env("DISPLAY", value);
            }
            None => {
                command.env_remove("DISPLAY");
            }
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("cannot connect to the X11 display server"),
            "{stderr}"
        );
        assert!(!stderr.contains("Wayland sessions"), "{stderr}");
        assert!(!stderr.contains("Usage:"), "{stderr}");
        if display.is_none_or(str::is_empty) {
            assert!(
                stderr.contains("DISPLAY is not set or is empty"),
                "{stderr}"
            );
        }
        if display == Some("127.0.0.1:65535") {
            assert!(stderr.contains("TCP port range"), "{stderr}");
        }
    }
}
