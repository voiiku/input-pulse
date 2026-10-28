use super::discovery::WindowInfo;
use crate::runtime::input_actions::{InputAction, Key, Modifier};
use std::time::Duration;

pub(crate) fn send_command(
    window: &WindowInfo,
    actions: &[InputAction],
    simulated_key_delay: Duration,
) -> Result<(), String> {
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let process_id = window.handle as libc::pid_t;
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "failed to create a macOS keyboard event source".to_owned())?;

    super::dispatch_actions(
        actions,
        simulated_key_delay,
        |character| post_macos_character(&source, process_id, character),
        |key, modifiers| post_macos_action(&source, process_id, key, modifiers),
    )
}

fn post_macos_key(
    source: &core_graphics::event_source::CGEventSource,
    process_id: libc::pid_t,
    key_code: core_graphics::event::CGKeyCode,
    key_down: bool,
    modifiers: &[Modifier],
) -> Result<(), String> {
    let event =
        core_graphics::event::CGEvent::new_keyboard_event(source.clone(), key_code, key_down)
            .map_err(|_| "failed to create a macOS keyboard event".to_owned())?;
    event.set_flags(macos_modifier_flags(modifiers));
    event.post_to_pid(process_id);
    Ok(())
}

fn post_macos_character(
    source: &core_graphics::event_source::CGEventSource,
    process_id: libc::pid_t,
    character: char,
) -> Result<(), String> {
    let mut buffer = [0; 4];
    let text = character.encode_utf8(&mut buffer);
    let key_down = core_graphics::event::CGEvent::new_keyboard_event(source.clone(), 0, true)
        .map_err(|_| "failed to create a macOS text key-down event".to_owned())?;
    key_down.set_string(text);
    key_down.post_to_pid(process_id);
    let key_up = core_graphics::event::CGEvent::new_keyboard_event(source.clone(), 0, false)
        .map_err(|_| "failed to create a macOS text key-up event".to_owned())?;
    key_up.set_string(text);
    key_up.post_to_pid(process_id);
    Ok(())
}

fn post_macos_action(
    source: &core_graphics::event_source::CGEventSource,
    process_id: libc::pid_t,
    key: Key,
    modifiers: &[Modifier],
) -> Result<(), String> {
    use core_graphics::event::KeyCode;

    let (key_code, shift_required) =
        macos_key(key).ok_or_else(|| format!("unsupported macOS simulated key: {key:?}"))?;
    let mut active_modifiers = modifiers.to_vec();
    if shift_required && !active_modifiers.contains(&Modifier::Shift) {
        active_modifiers.push(Modifier::Shift);
    }

    for (modifier_index, modifier) in modifiers.iter().enumerate() {
        post_macos_key(
            source,
            process_id,
            macos_modifier_key(*modifier),
            true,
            &active_modifiers[..=modifier_index],
        )?;
    }
    if shift_required && !modifiers.contains(&Modifier::Shift) {
        post_macos_key(source, process_id, KeyCode::SHIFT, true, &active_modifiers)?;
    }
    post_macos_key(source, process_id, key_code, true, &active_modifiers)?;
    post_macos_key(source, process_id, key_code, false, &active_modifiers)?;
    while let Some(modifier) = active_modifiers.pop() {
        post_macos_key(
            source,
            process_id,
            macos_modifier_key(modifier),
            false,
            &active_modifiers,
        )?;
    }
    Ok(())
}

fn macos_modifier_flags(modifiers: &[Modifier]) -> core_graphics::event::CGEventFlags {
    use core_graphics::event::CGEventFlags;

    modifiers.iter().fold(
        CGEventFlags::CGEventFlagNull,
        |flags, modifier| match modifier {
            Modifier::Control => flags | CGEventFlags::CGEventFlagControl,
            Modifier::Shift => flags | CGEventFlags::CGEventFlagShift,
            Modifier::Alt => flags | CGEventFlags::CGEventFlagAlternate,
            Modifier::Command => flags | CGEventFlags::CGEventFlagCommand,
        },
    )
}

fn macos_modifier_key(modifier: Modifier) -> core_graphics::event::CGKeyCode {
    use core_graphics::event::KeyCode;

    match modifier {
        Modifier::Control => KeyCode::CONTROL,
        Modifier::Shift => KeyCode::SHIFT,
        Modifier::Alt => KeyCode::OPTION,
        Modifier::Command => KeyCode::COMMAND,
    }
}

fn macos_key(key: Key) -> Option<(core_graphics::event::CGKeyCode, bool)> {
    use core_graphics::event::KeyCode;

    match key {
        Key::Character(character) => macos_key_code(character),
        Key::Enter => Some((KeyCode::RETURN, false)),
        Key::Tab => Some((KeyCode::TAB, false)),
        Key::Escape => Some((KeyCode::ESCAPE, false)),
        Key::Backspace => Some((KeyCode::DELETE, false)),
        Key::Delete => Some((KeyCode::FORWARD_DELETE, false)),
        Key::Up => Some((KeyCode::UP_ARROW, false)),
        Key::Down => Some((KeyCode::DOWN_ARROW, false)),
        Key::Left => Some((KeyCode::LEFT_ARROW, false)),
        Key::Right => Some((KeyCode::RIGHT_ARROW, false)),
        Key::Home => Some((KeyCode::HOME, false)),
        Key::End => Some((KeyCode::END, false)),
        Key::PageUp => Some((KeyCode::PAGE_UP, false)),
        Key::PageDown => Some((KeyCode::PAGE_DOWN, false)),
    }
}

fn macos_key_code(character: char) -> Option<(core_graphics::event::CGKeyCode, bool)> {
    use core_graphics::event::KeyCode;

    let (key_code, shift_required) = match character {
        'a' | 'A' => (KeyCode::ANSI_A, character.is_ascii_uppercase()),
        'b' | 'B' => (KeyCode::ANSI_B, character.is_ascii_uppercase()),
        'c' | 'C' => (KeyCode::ANSI_C, character.is_ascii_uppercase()),
        'd' | 'D' => (KeyCode::ANSI_D, character.is_ascii_uppercase()),
        'e' | 'E' => (KeyCode::ANSI_E, character.is_ascii_uppercase()),
        'f' | 'F' => (KeyCode::ANSI_F, character.is_ascii_uppercase()),
        'g' | 'G' => (KeyCode::ANSI_G, character.is_ascii_uppercase()),
        'h' | 'H' => (KeyCode::ANSI_H, character.is_ascii_uppercase()),
        'i' | 'I' => (KeyCode::ANSI_I, character.is_ascii_uppercase()),
        'j' | 'J' => (KeyCode::ANSI_J, character.is_ascii_uppercase()),
        'k' | 'K' => (KeyCode::ANSI_K, character.is_ascii_uppercase()),
        'l' | 'L' => (KeyCode::ANSI_L, character.is_ascii_uppercase()),
        'm' | 'M' => (KeyCode::ANSI_M, character.is_ascii_uppercase()),
        'n' | 'N' => (KeyCode::ANSI_N, character.is_ascii_uppercase()),
        'o' | 'O' => (KeyCode::ANSI_O, character.is_ascii_uppercase()),
        'p' | 'P' => (KeyCode::ANSI_P, character.is_ascii_uppercase()),
        'q' | 'Q' => (KeyCode::ANSI_Q, character.is_ascii_uppercase()),
        'r' | 'R' => (KeyCode::ANSI_R, character.is_ascii_uppercase()),
        's' | 'S' => (KeyCode::ANSI_S, character.is_ascii_uppercase()),
        't' | 'T' => (KeyCode::ANSI_T, character.is_ascii_uppercase()),
        'u' | 'U' => (KeyCode::ANSI_U, character.is_ascii_uppercase()),
        'v' | 'V' => (KeyCode::ANSI_V, character.is_ascii_uppercase()),
        'w' | 'W' => (KeyCode::ANSI_W, character.is_ascii_uppercase()),
        'x' | 'X' => (KeyCode::ANSI_X, character.is_ascii_uppercase()),
        'y' | 'Y' => (KeyCode::ANSI_Y, character.is_ascii_uppercase()),
        'z' | 'Z' => (KeyCode::ANSI_Z, character.is_ascii_uppercase()),
        '1' | '!' => (KeyCode::ANSI_1, character == '!'),
        '2' | '@' => (KeyCode::ANSI_2, character == '@'),
        '3' | '#' => (KeyCode::ANSI_3, character == '#'),
        '4' | '$' => (KeyCode::ANSI_4, character == '$'),
        '5' | '%' => (KeyCode::ANSI_5, character == '%'),
        '6' | '^' => (KeyCode::ANSI_6, character == '^'),
        '7' | '&' => (KeyCode::ANSI_7, character == '&'),
        '8' | '*' => (KeyCode::ANSI_8, character == '*'),
        '9' | '(' => (KeyCode::ANSI_9, character == '('),
        '0' | ')' => (KeyCode::ANSI_0, character == ')'),
        '-' | '_' => (KeyCode::ANSI_MINUS, character == '_'),
        '=' | '+' => (KeyCode::ANSI_EQUAL, character == '+'),
        '[' | '{' => (KeyCode::ANSI_LEFT_BRACKET, character == '{'),
        ']' | '}' => (KeyCode::ANSI_RIGHT_BRACKET, character == '}'),
        '\\' | '|' => (KeyCode::ANSI_BACKSLASH, character == '|'),
        ';' | ':' => (KeyCode::ANSI_SEMICOLON, character == ':'),
        '\'' | '"' => (KeyCode::ANSI_QUOTE, character == '"'),
        ',' | '<' => (KeyCode::ANSI_COMMA, character == '<'),
        '.' | '>' => (KeyCode::ANSI_PERIOD, character == '>'),
        '/' | '?' => (KeyCode::ANSI_SLASH, character == '?'),
        '`' | '~' => (KeyCode::ANSI_GRAVE, character == '~'),
        ' ' => (KeyCode::SPACE, false),
        _ => return None,
    };
    Some((key_code, shift_required))
}
