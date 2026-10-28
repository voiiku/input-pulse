#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub handle: usize,
    pub title: String,
}

pub fn find_windows(_title_match: &str) -> Result<Vec<WindowInfo>, String> {
    #[cfg(target_os = "windows")]
    {
        use winapi::shared::minwindef::{BOOL, LPARAM};
        use winapi::shared::windef::HWND;
        use winapi::um::winuser::EnumWindows;

        struct SearchContext {
            windows: Vec<WindowInfo>,
            title_match: String,
        }

        unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            use winapi::shared::minwindef::TRUE;
            use winapi::um::winuser::{GetWindowTextW, IsWindowVisible};

            unsafe {
                if IsWindowVisible(hwnd) == 0 {
                    return TRUE;
                }

                let mut title: [u16; 512] = [0; 512];
                let len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
                if len > 0 {
                    let title = String::from_utf16_lossy(&title[..len as usize]);
                    let context = &mut *(lparam as *mut SearchContext);
                    if title.contains(context.title_match.as_str()) {
                        context.windows.push(WindowInfo {
                            handle: hwnd as usize,
                            title,
                        });
                    }
                }

                TRUE
            }
        }

        let mut context = SearchContext {
            windows: Vec::new(),
            title_match: _title_match.to_owned(),
        };
        unsafe {
            EnumWindows(
                Some(enum_windows_proc),
                &mut context as *mut SearchContext as LPARAM,
            );
        }
        Ok(context.windows)
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let script = r#"
tell application "System Events"
    set outputText to ""
    repeat with currentProcess in (every process whose background only is false)
        try
            set processId to unix id of currentProcess
            set processWindows to windows of currentProcess
            repeat with windowIndex from 1 to count of processWindows
                set windowTitle to name of window windowIndex of currentProcess
                set outputText to outputText & (processId as text) & tab & (windowIndex as text) & tab & windowTitle & linefeed
            end repeat
        end try
    end repeat
    return outputText
end tell
"#;

        let output = Command::new("/usr/bin/osascript")
            .args(["-e", script])
            .output()
            .map_err(|error| {
                format!("macOS automation failed while enumerating windows: {error}")
            })?;
        if !output.status.success() {
            let details = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(format!(
                "macOS automation failed while enumerating windows; grant Accessibility, Automation, and Input Monitoring permissions to the host application ({details})"
            ));
        }
        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| format!("macOS window list was not valid UTF-8: {error}"))?;

        Ok(stdout
            .lines()
            .filter_map(|line| {
                let mut fields = line.splitn(3, '\t');
                let process_id = fields.next()?.parse::<usize>().ok()?;
                let _window_index = fields.next()?.parse::<usize>().ok()?;
                let title = fields.next()?.trim_end_matches('\r');
                (!title.is_empty() && title.contains(_title_match)).then(|| WindowInfo {
                    handle: process_id,
                    title: title.to_owned(),
                })
            })
            .collect())
    }

    #[cfg(target_os = "linux")]
    {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt};

        let (connection, screen_num) = x11rb::connect(None).map_err(super::x11_error)?;
        let root = connection.setup().roots[screen_num].root;

        let atom = |name: &[u8]| -> Result<Atom, String> {
            connection
                .intern_atom(false, name)
                .map_err(super::x11_error)?
                .reply()
                .map(|reply| reply.atom)
                .map_err(super::x11_error)
        };

        let client_list_atom = atom(b"_NET_CLIENT_LIST")?;
        let client_windows = connection
            .get_property(false, root, client_list_atom, AtomEnum::WINDOW, 0, 4096)
            .map_err(super::x11_error)?
            .reply()
            .map_err(super::x11_error)?
            .value32()
            .map(|windows| windows.collect::<Vec<_>>())
            .unwrap_or_default();

        let windows = if client_windows.is_empty() {
            connection
                .query_tree(root)
                .map_err(super::x11_error)?
                .reply()
                .map_err(super::x11_error)?
                .children
        } else {
            client_windows
        };

        let net_wm_name_atom = atom(b"_NET_WM_NAME")?;
        let utf8_string_atom = atom(b"UTF8_STRING")?;

        let mut matching_windows = Vec::new();
        for window in windows {
            let title_reply = connection
                .get_property(false, window, net_wm_name_atom, utf8_string_atom, 0, 1024)
                .map_err(super::x11_error)?
                .reply()
                .map_err(super::x11_error)?;
            let title = if title_reply.value.is_empty() {
                connection
                    .get_property(false, window, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 1024)
                    .map_err(super::x11_error)?
                    .reply()
                    .map_err(super::x11_error)?
                    .value
            } else {
                title_reply.value
            };
            let title = String::from_utf8_lossy(&title)
                .trim_matches(char::from(0))
                .to_owned();

            if !title.is_empty() && title.contains(_title_match) {
                matching_windows.push(WindowInfo {
                    handle: window as usize,
                    title,
                });
            }
        }

        Ok(matching_windows)
    }
}
