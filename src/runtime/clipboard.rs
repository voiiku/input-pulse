#[cfg(target_os = "windows")]
pub(crate) fn read_text() -> Result<String, String> {
    use std::{ptr, slice};
    use winapi::shared::minwindef::HGLOBAL;
    use winapi::um::errhandlingapi::{GetLastError, SetLastError};
    use winapi::um::winbase::{GlobalLock, GlobalSize, GlobalUnlock};
    use winapi::um::winuser::{CF_UNICODETEXT, CloseClipboard, GetClipboardData, OpenClipboard};

    if unsafe { OpenClipboard(ptr::null_mut()) } == 0 {
        return Err(format!(
            "failed to open clipboard: {}",
            std::io::Error::last_os_error()
        ));
    }

    let result = (|| {
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT) } as HGLOBAL;
        if handle.is_null() {
            return Err(format!(
                "clipboard does not contain Unicode text: {}",
                std::io::Error::last_os_error()
            ));
        }
        let size = unsafe { GlobalSize(handle) };
        if size < 2 || size % 2 != 0 || size > isize::MAX as usize {
            return Err("invalid clipboard Unicode text size".to_owned());
        }
        let data = unsafe { GlobalLock(handle) } as *const u16;
        if data.is_null() {
            return Err(format!(
                "failed to lock clipboard text: {}",
                std::io::Error::last_os_error()
            ));
        }
        let units = unsafe { slice::from_raw_parts(data, size / 2) };
        let text = match units.iter().position(|unit| *unit == 0) {
            Some(end) => String::from_utf16(&units[..end])
                .map_err(|error| format!("clipboard text is not valid UTF-16: {error}")),
            None => Err("clipboard Unicode text is not terminated".to_owned()),
        };
        unsafe { SetLastError(0) };
        if unsafe { GlobalUnlock(handle) } == 0 {
            let code = unsafe { GetLastError() };
            if code != 0 {
                return Err(format!("failed to unlock clipboard text: OS error {code}"));
            }
        }
        text
    })();

    let close_result = if unsafe { CloseClipboard() } == 0 {
        Err(format!(
            "failed to close clipboard: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    };
    match (result, close_result) {
        (Ok(text), Ok(())) => Ok(text),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(read_error), Err(close_error)) => Err(format!("{read_error}; {close_error}")),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn read_text() -> Result<String, String> {
    read_command("/usr/bin/pbpaste", &["-Prefer", "txt"])
}

#[cfg(target_os = "linux")]
pub(crate) fn read_text() -> Result<String, String> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        read_command("wl-paste", &["--no-newline", "--type", "text"])
    } else if std::env::var_os("DISPLAY").is_some() {
        read_command(
            "xclip",
            &["-selection", "clipboard", "-target", "UTF8_STRING", "-out"],
        )
    } else {
        Err(
            "clipboard input requires a graphical clipboard (DISPLAY or WAYLAND_DISPLAY)"
                .to_owned(),
        )
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn read_command(program: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run {program} to read clipboard: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} could not read clipboard ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("clipboard text is not valid UTF-8: {error}"))
}
