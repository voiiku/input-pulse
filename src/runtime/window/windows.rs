#[cfg(target_os = "windows")]
use super::discovery::WindowInfo;
use crate::runtime::input_actions::Modifier;
#[cfg(target_os = "windows")]
use crate::runtime::input_actions::{InputAction, Key};
#[cfg(target_os = "windows")]
use std::time::Duration;

#[cfg(target_os = "windows")]
pub(crate) fn send_command(
    window: &WindowInfo,
    actions: &[InputAction],
    simulated_key_delay: Duration,
) -> Result<(), String> {
    use winapi::shared::minwindef::{LPARAM, WPARAM};
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::{
        MAPVK_VK_TO_VSC, MapVirtualKeyW, PostMessageW, WM_CHAR, WM_KEYDOWN, WM_KEYUP,
        WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    let window_handle = window.handle as HWND;
    let post_key = |key: u8, down: bool, alt: bool| -> Result<(), String> {
        let message = match (down, alt) {
            (true, false) => WM_KEYDOWN,
            (false, false) => WM_KEYUP,
            (true, true) => WM_SYSKEYDOWN,
            (false, true) => WM_SYSKEYUP,
        };
        let scan = unsafe { MapVirtualKeyW(key as u32, MAPVK_VK_TO_VSC) };
        let extended = matches!(key, 0x21..=0x28 | 0x2d | 0x2e);
        let lparam = 1u32
            | ((scan & 0xff) << 16)
            | (u32::from(extended) << 24)
            | (u32::from(alt) << 29)
            | (u32::from(!down) << 30)
            | (u32::from(!down) << 31);
        if unsafe { PostMessageW(window_handle, message, key as WPARAM, lparam as LPARAM) } == 0 {
            Err(format!(
                "failed to send virtual key {key:#x} to window ({})",
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(())
        }
    };
    let post_character = |character: char| -> Result<(), String> {
        let mut units = [0; 2];
        for unit in character.encode_utf16(&mut units) {
            if unsafe { PostMessageW(window_handle, WM_CHAR, *unit as WPARAM, 1) } == 0 {
                return Err(format!(
                    "failed to send Unicode text to window ({})",
                    std::io::Error::last_os_error()
                ));
            }
        }
        Ok(())
    };
    let send_key = |key: Key, modifiers: &[Modifier]| -> Result<(), String> {
        let (virtual_key, shift_required) = windows_virtual_key(key)?;
        let alt = modifiers.contains(&Modifier::Alt) && !modifiers.contains(&Modifier::Control);
        let mut pressed = Vec::new();
        for (key, down) in windows_key_sequence(virtual_key, shift_required, modifiers)? {
            if let Err(error) = post_key(key, down, alt) {
                let mut cleanup_errors = Vec::new();
                while let Some(pressed_key) = pressed.pop() {
                    if let Err(cleanup_error) = post_key(pressed_key, false, alt) {
                        cleanup_errors.push(cleanup_error);
                    }
                }
                return if cleanup_errors.is_empty() {
                    Err(error)
                } else {
                    Err(format!(
                        "{error}; failed to release keys: {}",
                        cleanup_errors.join("; ")
                    ))
                };
            }
            if down {
                pressed.push(key);
            } else if let Some(position) =
                pressed.iter().rposition(|pressed_key| *pressed_key == key)
            {
                pressed.remove(position);
            }
        }
        Ok(())
    };

    super::dispatch_actions(actions, simulated_key_delay, post_character, send_key)
}

#[cfg(target_os = "windows")]
fn windows_virtual_key(key: Key) -> Result<(u8, bool), String> {
    use winapi::um::winuser::VkKeyScanW;

    let virtual_key = match key {
        Key::Enter => 0x0d,
        Key::Tab => 0x09,
        Key::Escape => 0x1b,
        Key::Backspace => 0x08,
        Key::Delete => 0x2e,
        Key::Up => 0x26,
        Key::Down => 0x28,
        Key::Left => 0x25,
        Key::Right => 0x27,
        Key::Home => 0x24,
        Key::End => 0x23,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::Character(character) => {
            let key = unsafe { VkKeyScanW(character as u16) };
            if key == -1 {
                return Err(format!("unsupported Windows simulated key: {character:?}"));
            }
            return Ok((key as u8, ((key as u16 >> 8) & 1) != 0));
        }
    };
    Ok((virtual_key, false))
}

#[cfg(any(test, target_os = "windows"))]
fn windows_modifier_key(modifier: Modifier) -> Result<u8, String> {
    match modifier {
        Modifier::Control => Ok(0x11),
        Modifier::Shift => Ok(0x10),
        Modifier::Alt => Ok(0x12),
        Modifier::Command => {
            Err("the COMMAND modifier is only supported by macOS window targets".to_owned())
        }
    }
}

#[cfg(any(test, target_os = "windows"))]
fn windows_key_sequence(
    key: u8,
    shift_required: bool,
    modifiers: &[Modifier],
) -> Result<Vec<(u8, bool)>, String> {
    let mut events = Vec::with_capacity((modifiers.len() + 1) * 2 + 2);
    for modifier in modifiers {
        events.push((windows_modifier_key(*modifier)?, true));
    }
    if shift_required && !modifiers.contains(&Modifier::Shift) {
        events.push((0x10, true));
    }
    let modifier_count = events.len();
    events.extend([(key, true), (key, false)]);
    for index in (0..modifier_count).rev() {
        events.push((events[index].0, false));
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::{Modifier, windows_key_sequence};

    #[test]
    fn windows_chord_keeps_modifiers_down_until_after_the_key() {
        assert_eq!(
            windows_key_sequence(0x56, false, &[Modifier::Control]).unwrap(),
            vec![(0x11, true), (0x56, true), (0x56, false), (0x11, false)]
        );
        assert_eq!(
            windows_key_sequence(0x5a, false, &[Modifier::Control, Modifier::Shift]).unwrap(),
            vec![
                (0x11, true),
                (0x10, true),
                (0x5a, true),
                (0x5a, false),
                (0x10, false),
                (0x11, false)
            ]
        );
        assert_eq!(
            windows_key_sequence(0x31, true, &[Modifier::Control]).unwrap(),
            vec![
                (0x11, true),
                (0x10, true),
                (0x31, true),
                (0x31, false),
                (0x10, false),
                (0x11, false)
            ]
        );
        assert_eq!(
            windows_key_sequence(0x31, true, &[Modifier::Shift]).unwrap(),
            vec![(0x10, true), (0x31, true), (0x31, false), (0x10, false)]
        );
        assert!(windows_key_sequence(0x56, false, &[Modifier::Command]).is_err());
    }
}
