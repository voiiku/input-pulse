#![cfg(all(feature = "cli", target_os = "linux"))]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "input-pulse-clipboard-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        let fixture = Self(path);
        fixture.write_script(
            "xclip",
            r#"#!/bin/sh
if [ "$INPUT_PULSE_CLIPBOARD_FAIL" = "1" ]; then
    printf 'clipboard failure\n' >&2
    exit 6
fi
count=0
[ ! -f "$TEST_DIR/reads" ] || read -r count < "$TEST_DIR/reads"
count=$((count + 1))
printf '%s\n' "$count" > "$TEST_DIR/reads"
printf 'read %s\n' "$count" >> "$TEST_DIR/log"
case "$count" in
    1) printf '!ENTER!' ;;
    2) printf 'cycle-one' ;;
    3) printf 'cycle-two' ;;
    *) exit 7 ;;
esac
"#,
        );
        fixture.write_script(
            "screen",
            r#"#!/bin/sh
case "$1" in
    -version) exit 0 ;;
    -list)
        count=0
        [ ! -f "$TEST_DIR/lists" ] || read -r count < "$TEST_DIR/lists"
        count=$((count + 1))
        printf '%s\n' "$count" > "$TEST_DIR/lists"
        printf 'list %s\n' "$count" >> "$TEST_DIR/log"
        case "$count" in
            1) printf 'No Sockets found in test directory.\n'; exit 1 ;;
            2|3)
                printf 'There are screens on:\n\t101.ip-test-one\t(Detached)\n\t102.ip-test-two\t(Detached)\n'
                exit 0
                ;;
            *) printf 'stop after test cycles\n' >&2; exit 2 ;;
        esac
        ;;
    -S)
        printf 'send %s: %s\n' "$2" "$5" >> "$TEST_DIR/log"
        ;;
    *) exit 9 ;;
esac
"#,
        );
        fixture
    }

    fn write_script(&self, name: &str, script: &str) {
        let path = self.0.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn run(&self, command: &str, fail: bool, background: bool) -> std::process::Output {
        let mut paths = vec![self.0.clone()];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        let mut process = Command::new(env!("CARGO_BIN_EXE_input-pulse"));
        if background {
            process.arg("--background");
        }
        process
            .args(["screen", "ip-test", command, "0", "0"])
            .env("TEST_DIR", &self.0)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("DISPLAY", ":input-pulse-test")
            .env_remove("WAYLAND_DISPLAY")
            .env("INPUT_PULSE_CLIPBOARD_FAIL", if fail { "1" } else { "0" })
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for name in ["xclip", "screen", "reads", "lists", "log"] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}

#[test]
fn startup_snapshot_and_cycle_reads_reach_all_screen_targets() {
    let fixture = Fixture::new();
    let output = fixture.run("!PASTE_ONCE!|!PASTE!!ENTER!", false, false);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("stop after test cycles"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Warning: clipboard text"));
    assert_eq!(
        fs::read_to_string(fixture.0.join("log")).unwrap(),
        concat!(
            "read 1\nlist 1\nlist 2\nread 2\n",
            "send 101.ip-test-one: !ENTER!|cycle-one\r\n",
            "send 102.ip-test-two: !ENTER!|cycle-one\r\n",
            "list 3\nread 3\n",
            "send 101.ip-test-one: !ENTER!|cycle-two\r\n",
            "send 102.ip-test-two: !ENTER!|cycle-two\r\n",
            "list 4\n"
        )
    );
}

#[test]
fn startup_clipboard_failure_stops_before_discovery() {
    let fixture = Fixture::new();
    let output = fixture.run("!PASTE_ONCE!", true, false);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("clipboard failure"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Warning: clipboard text"));
    assert!(!fixture.0.join("lists").exists());
}

#[test]
fn each_clipboard_mode_reads_only_at_its_expected_time() {
    for (command, expected) in [
        (
            "!PASTE_ONCE!",
            concat!(
                "read 1\nlist 1\nlist 2\n",
                "send 101.ip-test-one: !ENTER!\nsend 102.ip-test-two: !ENTER!\n",
                "list 3\nsend 101.ip-test-one: !ENTER!\nsend 102.ip-test-two: !ENTER!\nlist 4\n"
            ),
        ),
        (
            "!PASTE!",
            concat!(
                "list 1\nlist 2\nread 1\n",
                "send 101.ip-test-one: !ENTER!\nsend 102.ip-test-two: !ENTER!\n",
                "list 3\nread 2\n",
                "send 101.ip-test-one: cycle-one\nsend 102.ip-test-two: cycle-one\nlist 4\n"
            ),
        ),
    ] {
        let fixture = Fixture::new();
        let output = fixture.run(command, false, false);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("stop after test cycles"));
        assert_eq!(fs::read_to_string(fixture.0.join("log")).unwrap(), expected);
    }
}

#[test]
fn background_child_captures_the_startup_snapshot() {
    let fixture = Fixture::new();
    let output = fixture.run("!PASTE_ONCE!", false, true);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("PID"));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(log) = fs::read_to_string(fixture.0.join("log"))
            && log.ends_with("list 4\n")
        {
            assert_eq!(
                log,
                concat!(
                    "read 1\nlist 1\nlist 2\n",
                    "send 101.ip-test-one: !ENTER!\nsend 102.ip-test-two: !ENTER!\n",
                    "list 3\nsend 101.ip-test-one: !ENTER!\nsend 102.ip-test-two: !ENTER!\nlist 4\n"
                )
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background child did not complete"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
