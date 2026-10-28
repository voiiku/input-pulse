# Input Pulse

Input Pulse is a Rust CLI and library that sends input to matching windows or GNU screen sessions at regular intervals.

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
input-pulse = { version = "0.1.0", default-features = false }
```

```rust
use input_pulse::{InputPulseConfig, TargetMode};

fn main() -> Result<(), String> {
    let config = InputPulseConfig::new(
        TargetMode::Window, "Example Editor", "hello", 60, 0.05,
    )?;
    input_pulse::run(config, |_| {})
}
```

`InputPulseConfig::new` requires all input settings.
`run` blocks and reports progress through the callback without writing to stdout or stderr.
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
