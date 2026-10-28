use super::discovery::WindowInfo;
use crate::runtime::input_actions::{InputAction, Key, Modifier};
use std::time::Duration;

pub(crate) fn send_command(
    window: &WindowInfo,
    actions: &[InputAction],
    simulated_key_delay: Duration,
) -> Result<(), String> {
    use std::thread;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{
        ConnectionExt, EventMask, KEY_PRESS_EVENT, KEY_RELEASE_EVENT, KeyPressEvent, Window,
    };
    use x11rb::{CURRENT_TIME, NONE};

    let (connection, screen_num) = x11rb::connect(None).map_err(super::x11_error)?;
    let root = connection.setup().roots[screen_num].root;
    let target = window.handle as Window;
    let keysyms = connection
        .get_keyboard_mapping(
            connection.setup().min_keycode,
            connection.setup().max_keycode - connection.setup().min_keycode + 1,
        )
        .map_err(super::x11_error)?
        .reply()
        .map_err(super::x11_error)?;
    let resolved = resolve_actions(
        actions,
        &keysyms.keysyms,
        keysyms.keysyms_per_keycode as usize,
        connection.setup().min_keycode,
    )?;
    for (keycode, state) in resolved {
        for response_type in [KEY_PRESS_EVENT, KEY_RELEASE_EVENT] {
            let event = KeyPressEvent {
                response_type,
                detail: keycode,
                sequence: 0,
                time: CURRENT_TIME,
                root,
                event: target,
                child: NONE,
                root_x: 0,
                root_y: 0,
                event_x: 0,
                event_y: 0,
                state,
                same_screen: true,
            };
            connection
                .send_event(false, target, EventMask::NO_EVENT, event)
                .map_err(super::x11_error)?
                .check()
                .map_err(super::x11_error)?;
        }
        connection.flush().map_err(super::x11_error)?;
        thread::sleep(simulated_key_delay);
    }
    Ok(())
}

fn resolve_actions(
    actions: &[InputAction],
    keysyms: &[u32],
    keysyms_per_keycode: usize,
    min_keycode: u8,
) -> Result<Vec<(u8, x11rb::protocol::xproto::KeyButMask)>, String> {
    if keysyms_per_keycode == 0 {
        return Err(
            "X11 operation failed: the current keyboard mapping requires an unsupported modifier combination"
                .to_owned(),
        );
    }
    let mut resolved = Vec::new();
    let mut push_key = |keysym: u32, modifiers: &[Modifier]| -> Result<(), String> {
        let (keycode, level) = keysyms
            .chunks(keysyms_per_keycode)
            .enumerate()
            .find_map(|(offset, symbols)| {
                symbols
                    .iter()
                    .position(|symbol| *symbol == keysym)
                    .map(|level| (min_keycode + offset as u8, level))
            })
            .ok_or_else(|| {
                "X11 operation failed: the required key was not found in the current keyboard mapping"
                    .to_owned()
            })?;
        if level > 1 {
            return Err("X11 operation failed: the current keyboard mapping requires an unsupported modifier combination".to_owned());
        }
        resolved.push((keycode, x11_event_state(level, modifiers)?));
        Ok(())
    };
    for action in actions {
        match action {
            InputAction::Text(text) => {
                for character in text.chars() {
                    push_key(x11_character_keysym(character), &[])?;
                }
            }
            InputAction::Key(key) => push_key(x11_key_keysym(*key), &[])?,
            InputAction::Chord { key, modifiers } => push_key(x11_key_keysym(*key), modifiers)?,
            InputAction::Paste(_) => return Err("clipboard input was not resolved".to_owned()),
        }
    }
    Ok(resolved)
}

fn x11_character_keysym(character: char) -> u32 {
    if (character as u32) <= 0xff {
        character as u32
    } else {
        0x0100_0000 | character as u32
    }
}

fn x11_event_state(
    level: usize,
    modifiers: &[Modifier],
) -> Result<x11rb::protocol::xproto::KeyButMask, String> {
    use x11rb::protocol::xproto::KeyButMask;
    let mut state = KeyButMask::from(0u16);
    if level == 1 {
        state |= KeyButMask::SHIFT;
    }
    for modifier in modifiers {
        state |= match modifier {
            Modifier::Control => KeyButMask::CONTROL,
            Modifier::Shift => KeyButMask::SHIFT,
            Modifier::Alt => KeyButMask::MOD1,
            Modifier::Command => {
                return Err(
                    "the COMMAND modifier is only supported by macOS window targets".to_owned(),
                );
            }
        };
    }
    Ok(state)
}

fn x11_key_keysym(key: Key) -> u32 {
    match key {
        Key::Character(character) => x11_character_keysym(character),
        Key::Enter => 0xff0d,
        Key::Tab => 0xff09,
        Key::Escape => 0xff1b,
        Key::Backspace => 0xff08,
        Key::Delete => 0xffff,
        Key::Up => 0xff52,
        Key::Down => 0xff54,
        Key::Left => 0xff51,
        Key::Right => 0xff53,
        Key::Home => 0xff50,
        Key::End => 0xff57,
        Key::PageUp => 0xff55,
        Key::PageDown => 0xff56,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InputAction, Key, Modifier, resolve_actions, x11_character_keysym, x11_event_state,
    };
    use x11rb::protocol::xproto::{EventMask, KeyButMask};

    #[test]
    #[ignore = "requires an isolated X11 server and INPUT_PULSE_TEST_X11_DISPLAY"]
    fn x11_delivers_to_target_without_changing_focus() {
        use super::{WindowInfo, send_command};
        use std::{env, time::Duration};
        use x11rb::{
            COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME,
            connection::Connection,
            protocol::{
                Event,
                xproto::{
                    ChangeWindowAttributesAux, ConnectionExt, CreateWindowAux, InputFocus,
                    WindowClass,
                },
            },
        };

        assert_eq!(
            env::var("DISPLAY").unwrap(),
            env::var("INPUT_PULSE_TEST_X11_DISPLAY").unwrap()
        );
        let (connection, screen_num) = x11rb::connect(None).unwrap();
        let (observer, _) = x11rb::connect(None).unwrap();
        let root = connection.setup().roots[screen_num].root;
        let create_window = |mask| {
            let window = connection.generate_id().unwrap();
            connection
                .create_window(
                    COPY_DEPTH_FROM_PARENT,
                    window,
                    root,
                    0,
                    0,
                    100,
                    100,
                    0,
                    WindowClass::INPUT_OUTPUT,
                    COPY_FROM_PARENT,
                    &CreateWindowAux::new().event_mask(mask),
                )
                .unwrap()
                .check()
                .unwrap();
            connection.map_window(window).unwrap().check().unwrap();
            window
        };
        let decoy = create_window(EventMask::KEY_PRESS | EventMask::KEY_RELEASE);
        connection
            .set_input_focus(InputFocus::PARENT, decoy, CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();

        for mask in [
            EventMask::NO_EVENT,
            EventMask::KEY_PRESS | EventMask::KEY_RELEASE,
        ] {
            let target = create_window(mask);
            observer
                .change_window_attributes(
                    target,
                    &ChangeWindowAttributesAux::new()
                        .event_mask(EventMask::KEY_PRESS | EventMask::KEY_RELEASE),
                )
                .unwrap()
                .check()
                .unwrap();
            send_command(
                &WindowInfo {
                    handle: target as usize,
                    title: String::new(),
                },
                &[InputAction::Text("ab".into())],
                Duration::ZERO,
            )
            .unwrap();
            assert_eq!(
                connection.get_input_focus().unwrap().reply().unwrap().focus,
                decoy
            );
            let mut events = Vec::new();
            while let Some(event) = connection.poll_for_event().unwrap() {
                match event {
                    Event::KeyPress(event) => {
                        assert_eq!(event.event, target);
                        assert_ne!(event.response_type & 0x80, 0);
                        events.push((true, event.detail));
                    }
                    Event::KeyRelease(event) => {
                        assert_eq!(event.event, target);
                        assert_ne!(event.response_type & 0x80, 0);
                        events.push((false, event.detail));
                    }
                    _ => {}
                }
            }
            assert_eq!(events.len(), 4);
            assert!(events[0].0 && !events[1].0 && events[2].0 && !events[3].0);
            assert_eq!(events[0].1, events[1].1);
            assert_eq!(events[2].1, events[3].1);
            assert_ne!(events[0].1, events[2].1);
            observer.get_input_focus().unwrap().reply().unwrap();
            assert!(observer.poll_for_event().unwrap().is_none());
        }
    }

    #[test]
    #[ignore = "requires an isolated X11 server and INPUT_PULSE_TEST_X11_DISPLAY"]
    fn x11_sends_to_every_window_with_the_same_title() {
        use crate::{RunEvent, runtime::executor};
        use std::{env, time::Duration};
        use x11rb::{
            COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME,
            connection::Connection,
            protocol::{
                Event,
                xproto::{AtomEnum, ConnectionExt, CreateWindowAux, InputFocus, WindowClass},
            },
            wrapper::ConnectionExt as _,
        };

        assert_eq!(
            env::var("DISPLAY").unwrap(),
            env::var("INPUT_PULSE_TEST_X11_DISPLAY").unwrap()
        );
        let (connection, screen_num) = x11rb::connect(None).unwrap();
        let root = connection.setup().roots[screen_num].root;
        let create_window = |title: &str| {
            let window = connection.generate_id().unwrap();
            connection
                .create_window(
                    COPY_DEPTH_FROM_PARENT,
                    window,
                    root,
                    0,
                    0,
                    100,
                    100,
                    0,
                    WindowClass::INPUT_OUTPUT,
                    COPY_FROM_PARENT,
                    &CreateWindowAux::new(),
                )
                .unwrap()
                .check()
                .unwrap();
            connection
                .change_property8(
                    x11rb::protocol::xproto::PropMode::REPLACE,
                    window,
                    AtomEnum::WM_NAME,
                    AtomEnum::STRING,
                    title.as_bytes(),
                )
                .unwrap()
                .check()
                .unwrap();
            connection.map_window(window).unwrap().check().unwrap();
            window
        };
        let first = create_window("Input Pulse Same Title");
        let second = create_window("Input Pulse Same Title");
        let decoy = create_window("Input Pulse Unrelated");
        connection
            .set_input_focus(InputFocus::PARENT, decoy, CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();

        let windows = super::super::find_windows("Input Pulse Same Title").unwrap();
        assert_eq!(windows.len(), 2);
        assert!(windows.iter().any(|window| window.handle == first as usize));
        assert!(
            windows
                .iter()
                .any(|window| window.handle == second as usize)
        );
        let mut events = Vec::new();
        executor::execute_for_windows(
            &windows,
            &[InputAction::Text("ab".into())],
            Duration::ZERO,
            &mut |event| events.push(event),
        )
        .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, RunEvent::CommandComplete))
                .count(),
            2
        );
        assert_eq!(
            connection.get_input_focus().unwrap().reply().unwrap().focus,
            decoy
        );

        let mut received = Vec::new();
        while let Some(event) = connection.poll_for_event().unwrap() {
            match event {
                Event::KeyPress(event) => received.push((event.event, true, event.detail)),
                Event::KeyRelease(event) => received.push((event.event, false, event.detail)),
                _ => {}
            }
        }
        let target_keys = [first, second].map(|target| {
            received
                .iter()
                .filter(|(window, _, _)| *window == target)
                .map(|(_, down, key)| (*down, *key))
                .collect::<Vec<_>>()
        });
        for keys in &target_keys {
            assert_eq!(keys.len(), 4);
            assert!(keys[0].0 && !keys[1].0 && keys[2].0 && !keys[3].0);
            assert_eq!(keys[0].1, keys[1].1);
            assert_eq!(keys[2].1, keys[3].1);
            assert_ne!(keys[0].1, keys[2].1);
        }
        assert_eq!(target_keys[0], target_keys[1]);
        assert_eq!(received.len(), 8);
    }

    #[test]
    fn x11_synthetic_events_carry_modifier_state() {
        let state = x11_event_state(1, &[Modifier::Control, Modifier::Alt]).unwrap();
        assert_eq!(
            u16::from(state),
            u16::from(KeyButMask::SHIFT | KeyButMask::CONTROL | KeyButMask::MOD1)
        );
        assert_eq!(u16::from(x11_event_state(0, &[]).unwrap()), 0);
        assert!(x11_event_state(0, &[Modifier::Command]).is_err());
    }

    #[test]
    fn x11_uses_latin1_and_unicode_keysyms() {
        assert_eq!(x11_character_keysym('é'), 0xe9);
        assert_eq!(x11_character_keysym('你'), 0x0100_4f60);
    }

    #[test]
    fn resolves_all_input_before_delivery_in_order() {
        let actions = [
            InputAction::Text("aA".into()),
            InputAction::Chord {
                modifiers: vec![Modifier::Control],
                key: Key::Character('v'),
            },
            InputAction::Key(Key::Enter),
        ];
        let map = [
            b'a' as u32,
            b'A' as u32,
            b'v' as u32,
            b'V' as u32,
            0xff0d,
            0,
        ];
        let resolved = resolve_actions(&actions, &map, 2, 8).unwrap();
        assert_eq!(
            resolved,
            vec![
                (8, KeyButMask::from(0u16)),
                (8, KeyButMask::SHIFT),
                (9, KeyButMask::CONTROL),
                (10, KeyButMask::from(0u16)),
            ]
        );
        assert!(resolve_actions(&actions, &map[..4], 2, 8).is_err());
        assert!(resolve_actions(&actions, &map, 0, 8).is_err());
    }
}
