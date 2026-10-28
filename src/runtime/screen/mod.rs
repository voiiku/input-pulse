mod delivery;
mod discovery;
mod input;

pub(crate) use delivery::send_command;
pub(crate) use discovery::{ScreenSessionInfo, find_screen_sessions};
pub(crate) use input::{render_actions, validate_actions};

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn command_failure_detail(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut detail = output.status.to_string();
    for (name, value) in [("stdout", stdout.trim()), ("stderr", stderr.trim())] {
        if !value.is_empty() {
            detail.push_str(&format!("; {name}: {value}"));
        }
    }
    detail
}
