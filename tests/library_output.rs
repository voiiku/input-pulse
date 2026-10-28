#![cfg(any(target_os = "linux", target_os = "macos"))]

use input_pulse::{InputPulseConfig, RunEvent, TargetMode};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MODE: &str = "INPUT_PULSE_LIBRARY_OUTPUT_TEST_MODE";
const VERSION_STDOUT: &str = "input-pulse-test-version-stdout";
const VERSION_STDERR: &str = "input-pulse-test-version-stderr";
const LIST_ERROR: &str = "input-pulse-test-list-error";
const DELIVERY_STDOUT: &str = "input-pulse-test-delivery-stdout";
const DELIVERY_STDERR: &str = "input-pulse-test-delivery-stderr";

struct FakeScreen(PathBuf);

impl Drop for FakeScreen {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("screen"));
        let _ = fs::remove_file(self.0.join("version-count"));
        let _ = fs::remove_dir(&self.0);
    }
}

#[test]
fn library_does_not_inherit_screen_output() {
    if let Some(mode) = std::env::var_os(MODE) {
        run_probe(mode.to_str().expect("probe mode is valid UTF-8"));
        return;
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = FakeScreen(
        std::env::temp_dir().join(format!("input-pulse-output-{}-{nonce}", std::process::id())),
    );
    fs::create_dir(&fixture.0).unwrap();
    let executable = fixture.0.join("screen");
    fs::write(
        &executable,
        format!(
            r#"#!/bin/sh
case "$1" in
    -version)
        if [ -n "$INPUT_PULSE_VERSION_LOG" ]; then printf 'check\n' >> "$INPUT_PULSE_VERSION_LOG"; fi
        printf '%s\n' '{VERSION_STDOUT}'
        printf '%s\n' '{VERSION_STDERR}' >&2
        if [ "$INPUT_PULSE_LIBRARY_OUTPUT_TEST_MODE" = "version_failure" ] || [ "$INPUT_PULSE_LIBRARY_OUTPUT_TEST_MODE" = "environment_only_failure" ]; then exit 7; fi
        ;;
    -list)
        if [ "$INPUT_PULSE_LIBRARY_OUTPUT_TEST_MODE" = "list_failure" ]; then
            printf '%s\n' '{LIST_ERROR}'
            exit 1
        fi
        printf 'There are screens on:\n\t123.input-pulse-output-test\t(Detached)\n'
        ;;
    -S)
        printf '%s\n' '{DELIVERY_STDOUT}'
        printf '%s\n' '{DELIVERY_STDERR}' >&2
        if [ "$INPUT_PULSE_LIBRARY_OUTPUT_TEST_MODE" = "delivery_success" ]; then exit 0; fi
        exit 8
        ;;
    *) exit 9 ;;
esac
"#
        ),
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let mut paths = vec![fixture.0.clone()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").expect("PATH is available"),
    ));
    let path = std::env::join_paths(paths).unwrap();

    for mode in [
        "environment_only_success",
        "environment_only_failure",
        "version_failure",
        "delivery_failure",
        "delivery_success",
        "list_failure",
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "library_does_not_inherit_screen_output",
                "--nocapture",
            ])
            .env(MODE, mode)
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        for marker in [
            VERSION_STDOUT,
            VERSION_STDERR,
            DELIVERY_STDOUT,
            DELIVERY_STDERR,
        ] {
            assert!(!String::from_utf8_lossy(&output.stdout).contains(marker));
            assert!(!String::from_utf8_lossy(&output.stderr).contains(marker));
        }
    }

    #[cfg(feature = "cli")]
    {
        let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
            .args(["screen", "input-pulse-output-test", "status!ENTER!", "60"])
            .env(MODE, "version_failure")
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(VERSION_STDOUT));
        assert!(stderr.contains(VERSION_STDERR));
        assert!(!stderr.contains("Usage:"));
        assert!(output.stdout.is_empty());

        let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
            .args([
                "--background",
                "screen",
                "input-pulse-output-test",
                "status!ENTER!",
                "60",
            ])
            .env(MODE, "version_failure")
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(VERSION_STDERR));
        assert!(output.stdout.is_empty());

        let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
            .args(["screen", "input-pulse-output-test", "status!ENTER!", "60"])
            .env(MODE, "delivery_failure")
            .env("PATH", &path)
            .env("INPUT_PULSE_VERSION_LOG", fixture.0.join("version-count"))
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stdout.contains("Sending input"));
        assert!(!stdout.contains(VERSION_STDOUT));
        assert!(!stderr.contains(VERSION_STDERR));
        assert!(stderr.contains(DELIVERY_STDOUT));
        assert!(stderr.contains(DELIVERY_STDERR));
        assert_eq!(
            fs::read_to_string(fixture.0.join("version-count")).unwrap(),
            "check\n"
        );

        let output = Command::new(env!("CARGO_BIN_EXE_input-pulse"))
            .args(["screen", "input-pulse-output-test", "status!ENTER!", "60"])
            .env(MODE, "list_failure")
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(LIST_ERROR));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("No matching target found"));
    }
}

fn run_probe(mode: &str) {
    if mode.starts_with("environment_only_") {
        let result = input_pulse::check_platform_support(TargetMode::ScreenSession);
        match mode {
            "environment_only_success" => assert!(result.is_ok()),
            "environment_only_failure" => {
                let error = result.unwrap_err();
                assert!(error.contains(VERSION_STDOUT));
                assert!(error.contains(VERSION_STDERR));
            }
            _ => panic!("unknown probe mode: {mode}"),
        }
        return;
    }

    let config = InputPulseConfig::new(
        TargetMode::ScreenSession,
        "input-pulse-output-test",
        "status!ENTER!",
        60,
        0.05,
    )
    .unwrap();
    let mut events = Vec::new();
    if mode == "delivery_success" {
        struct StopProbe;
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            input_pulse::run(config, |event| {
                let complete = matches!(event, RunEvent::CommandComplete);
                events.push(event);
                if complete {
                    std::panic::panic_any(StopProbe);
                }
            })
        }));
        std::panic::set_hook(hook);
        match stopped {
            Err(payload) => assert!(payload.is::<StopProbe>()),
            Ok(result) => panic!("run returned unexpectedly: {result:?}"),
        }
        assert!(events.contains(&RunEvent::Started));
        assert!(events.contains(&RunEvent::CommandComplete));
        return;
    }

    let error = input_pulse::run(config, |event| events.push(event)).unwrap_err();
    match mode {
        "version_failure" => {
            assert!(error.contains(VERSION_STDOUT));
            assert!(error.contains(VERSION_STDERR));
            assert!(events.is_empty());
        }
        "delivery_failure" => {
            assert!(error.contains(DELIVERY_STDOUT));
            assert!(error.contains(DELIVERY_STDERR));
            assert!(events.contains(&RunEvent::Started));
            assert!(events.contains(&RunEvent::CommandStarted));
            assert!(!events.contains(&RunEvent::CommandComplete));
        }
        "list_failure" => {
            assert!(error.contains(LIST_ERROR));
            assert!(events.contains(&RunEvent::Started));
            assert!(!events.contains(&RunEvent::NoTargetFound));
        }
        _ => panic!("unknown probe mode: {mode}"),
    }
}
