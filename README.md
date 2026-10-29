# Input Pulse

Input Pulse is a CLI and library that sends input to targets at regular intervals.
![Window](/docs/assets/demo1.gif) 

Window input does not intentionally activate the target.
Applications may ignore synthetic input, so successful dispatch does not guarantee receipt.

| Target   | Windows | macOS |    Linux |
| -------- | ------: | ----: | -------: |
| `window` |     Yes |   Yes | X11 only |
| `screen` |      No |   Yes |      Yes |

Other operating systems are rejected.
Window input varies by platform:

- **Windows:** Sends keyboard and UTF-16 text messages; background shortcuts may not be recognized.
- **macOS:** Sends input to the target process, not a specific window, and some applications ignore background input.
  Multiple matching windows in the same process may cause repeated input to one window.
  Grant Accessibility and Automation permissions to the launching application; key actions use an ANSI keyboard mapping.
- **Linux:** Window mode requires X11; see [X11 and SSH](#x11-and-ssh).

The `screen` target requires GNU screen and works without a graphical session on macOS and Linux.
Session discovery supports `screen -list` output with or without timestamps; unrecognized output is reported as an error.
Avoid session names ending in a tab followed by a timestamp-like field, which `screen -list` cannot distinguish from a timestamp.

## Installation

Download a binary from [GitHub Releases](https://github.com/voiiku/input-pulse/releases), or install the CLI from [crates.io](https://crates.io/crates/input-pulse):

```sh
cargo install input-pulse
```

Ensure Cargo's binary directory is on `PATH`.
On Linux and macOS, use `chmod +x` on the downloaded binary if needed.

## Usage

### CLI usage

```text
input-pulse [options] <target> <target-match> <command> <interval-seconds> [key-delay-seconds]
```

| Argument              | Description                                                        |
| --------------------- | ------------------------------------------------------------------ |
| `<target>`            | `window` or `screen`.                                              |
| `<target-match>`      | Case-sensitive substring of a window title or screen session name. |
| `<command>`           | Text and optional [special input actions](#special-input-actions). |
| `<interval-seconds>`  | Whole seconds between completed cycles.                            |
| `[key-delay-seconds]` | Seconds after each character or key action; default `0.05`.        |

Input is sent to **all** matches, so use a unique target when testing.
Options must come before the target:

- `-h` / `--help`: display help.
- `-b` / `--background`: start a detached process and print its PID. Errors are not displayed because its standard streams are redirected to null.

Quote `<target-match>` and `<command>` as required by your shell; Windows CMD uses double quotes.
No Enter or newline is added automatically; include `!ENTER!` when needed.

### Library usage

Add the library without CLI dependencies:

```toml
[dependencies]
input-pulse = { version = "0.1.1", default-features = false }
```

```rust
use input_pulse::{InputPulse, InputPulseConfig, TargetMode};

fn main() -> Result<(), String> {
    let config = InputPulseConfig::new(
        TargetMode::Window, "Example Editor", "hello", 60, 0.05,
    )?;
    let mut pulse = InputPulse::new(config)?;
    pulse.start(|_| {})?;

    // When the host wants to stop:
    pulse.stop()
}
```

`InputPulseConfig::new` requires all input settings.
`start(on_event)` launches a background worker; callbacks must be `Send + 'static`.
Use `|_| {}` to ignore events. The library does not write to stdout or stderr.
`is_running()` reports thread liveness, not successful input delivery.

`stop()` wakes interval waits, finishes the current target's input, and joins the worker.
`wait()` joins without requesting a stop, for foreground use.
Both return worker errors or panics; call either before restarting or dropping the object.
Dropping only requests a stop without waiting. System calls and callbacks cannot be
interrupted, so stopping may take time; use a blocking task in async hosts.

The `help` and `background` fields are CLI-only.
See the runnable [window](examples/window.rs) and [screen](examples/screen.rs) examples.

### Special input actions

Commands can mix plain text with actions written between `!` characters.
For example, `!T!hello!ENTER!` presses T, types `hello`, and presses Enter.
Use `!!` for a literal `!`; unknown or incomplete actions are rejected.

| Syntax                                                                             | Action                                                                                |
| ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `!T!`, `!/!`, `!1!`                                                                | Press an ASCII key. Letter case names the same base key; use `!SHIFT+A!` for Shift+A. |
| `!ENTER!`, `!TAB!`, `!ESC!`, `!BACKSPACE!`, `!DELETE!`                             | Press a named key.                                                                    |
| `!UP!`, `!DOWN!`, `!LEFT!`, `!RIGHT!`, `!HOME!`, `!END!`, `!PAGEUP!`, `!PAGEDOWN!` | Press a navigation key.                                                               |
| `!CTRL+C!`, `!ALT+ENTER!`, `!COMMAND+V!`                                           | Press a key chord. `COMMAND` / `CMD` is macOS-only for window targets.                |
| `!PASTE!`                                                                          | Send clipboard text literally, without interpreting it as actions.                    |
| `!PASTE_ONCE!`                                                                     | Send a clipboard snapshot taken at startup.                                           |

`!PASTE!` reads the clipboard once per cycle when targets match; `!PASTE_ONCE!` reads it once at startup and reuses the same text.

Clipboard text may contain secrets, newlines or control characters that submit or run commands; NUL characters are rejected.
On Linux, clipboard access requires a graphical session and either `xclip` on X11 or `wl-paste` on Wayland.

For `screen`, key actions become terminal bytes rather than GUI key events.
`!ENTER!` sends a carriage return (`0x0d`), and `!CTRL+A!` sends `0x01`; their effects depend on the program in the session.
Only supported ASCII `CTRL` chords are accepted; `SHIFT`, `ALT`, `COMMAND` and NUL are rejected.
For faster delivery, set `key-delay-seconds` to `0` to send input in chunks without splitting UTF-8 characters.
An error may occur after some input has already been sent.

### Samples

```cmd
REM Windows CMD: start a new CMD with cmd /V:OFF if delayed expansion is enabled.
REM Send text to matching windows every 60 seconds.
input-pulse window "Example Editor" "hello" 60
```

```sh
# Linux X11 or macOS, POSIX shell: send text to matching windows every 60 seconds.
input-pulse window "Example Editor" 'hello' 60

# Linux or macOS, POSIX shell: send a screen command with a 0.01-second input delay.
input-pulse screen "worker_" 'status!ENTER!' 60 0.01

# Linux X11 or macOS, POSIX shell: run in the background every 60 seconds.
input-pulse --background window "Example App" 'keepalive!ENTER!' 60
```

## X11 and SSH

Linux window mode connects to the X11 display set by `DISPLAY`; explicit Wayland sessions are rejected.
Text and key actions are sent as synthetic key events, so characters must be available in the current keyboard mapping.
Events are sent to the client that created the matched window.
Some applications, including editors and terminals, may ignore these events.

When connecting over SSH, `ssh -X` and `ssh -Y` usually select a forwarded display, not the remote machine's existing desktop.
To target that desktop, check its display in a desktop terminal:

```sh
printf 'DISPLAY=%s\n' "$DISPLAY"
```

Then use that display from an authorized account in the SSH session.
For example, if the desktop uses `:0` and the account has access:

```sh
DISPLAY=:0 xdpyinfo >/dev/null && echo "X11 connected"
DISPLAY=:0 input-pulse window "Firefox" '123' 3
```

`xdpyinfo` is optional, and each `DISPLAY` assignment applies only to its own command.

## License

This project is licensed under the [MIT License](LICENSE).

Third-party licenses are listed in [THIRD-PARTY-LICENSES.html](https://github.com/voiiku/input-pulse/blob/main/THIRD-PARTY-LICENSES.html).
