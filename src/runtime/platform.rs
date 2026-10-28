use crate::TargetMode;

/// Checks whether a target mode is supported in the current environment.
///
/// This checks platform prerequisites without validating the input configuration
/// and does not write to stdout or stderr.
pub fn check_platform_support(target_mode: TargetMode) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        match target_mode {
            TargetMode::Window => Ok(()),
            TargetMode::ScreenSession => {
                Err("screen session mode is not supported on Windows".to_owned())
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if target_mode == TargetMode::ScreenSession {
            return check_screen_support();
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        if target_mode == TargetMode::ScreenSession {
            return check_screen_support();
        }

        use std::env;

        let session_type = env::var("XDG_SESSION_TYPE").ok();
        if is_wayland_session(
            session_type.as_deref(),
            env::var_os("WAYLAND_DISPLAY").is_some(),
        ) {
            return Err("Linux window mode does not support Wayland sessions".to_owned());
        }

        if env::var_os("DISPLAY").is_none_or(|display| display.is_empty()) {
            return Err("cannot connect to the X11 display server: DISPLAY is not set or is empty; for SSH, select an accessible X11 display and authorization".to_owned());
        }

        let display = x11rb::reexports::x11rb_protocol::parse_display::parse_display(None)
            .map_err(|error| format!("cannot connect to the X11 display server: {error}"))?;
        if x11_tcp_port_overflows(&display) {
            return Err(
                "cannot connect to the X11 display server: DISPLAY number exceeds the TCP port range"
                    .to_owned(),
            );
        }

        x11rb::connect(None)
            .map_err(|error| format!("cannot connect to the X11 display server: {error}"))?;

        Ok(())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn check_screen_support() -> Result<(), String> {
    let output = std::process::Command::new("screen")
        .arg("-version")
        .output()
        .map_err(|error| format!("failed to check GNU screen availability: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "GNU screen is unavailable ({})",
            super::screen::command_failure_detail(&output)
        ))
    }
}

#[cfg(target_os = "linux")]
fn is_wayland_session(session_type: Option<&str>, wayland_display_present: bool) -> bool {
    wayland_display_present
        || session_type.is_some_and(|session_type| session_type.eq_ignore_ascii_case("wayland"))
}

#[cfg(target_os = "linux")]
fn x11_tcp_port_overflows(
    display: &x11rb::reexports::x11rb_protocol::parse_display::ParsedDisplay,
) -> bool {
    // x11rb adds 6000 in u16 arithmetic while constructing TCP addresses.
    let tcp_possible = (display.protocol.as_deref() != Some("unix")
        && !display.host.is_empty()
        && display.host != "unix")
        || (display.protocol.is_none() && display.host.is_empty());
    tcp_possible && display.display.checked_add(6000).is_none()
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{is_wayland_session, x11_tcp_port_overflows};
    use x11rb::reexports::x11rb_protocol::parse_display::parse_display;

    #[test]
    fn rejects_explicit_wayland_but_allows_ssh_session_metadata() {
        assert!(is_wayland_session(Some("wayland"), false));
        assert!(is_wayland_session(Some("Wayland"), false));
        assert!(is_wayland_session(Some("x11"), true));
        assert!(is_wayland_session(None, true));
        assert!(!is_wayland_session(Some("x11"), false));
        assert!(!is_wayland_session(Some("X11"), false));
        assert!(!is_wayland_session(Some("tty"), false));
        assert!(!is_wayland_session(None, false));
        assert!(!is_wayland_session(Some(""), false));
    }

    #[test]
    fn validates_tcp_display_port_without_rejecting_unix_displays() {
        for value in [":59536", "127.0.0.1:65535"] {
            assert!(x11_tcp_port_overflows(&parse_display(Some(value)).unwrap()));
        }
        for value in [":59535", "127.0.0.1:10", "unix/:65535"] {
            assert!(!x11_tcp_port_overflows(
                &parse_display(Some(value)).unwrap()
            ));
        }
    }
}
