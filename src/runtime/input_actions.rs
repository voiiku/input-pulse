use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputAction {
    Text(String),
    Key(Key),
    Chord { modifiers: Vec<Modifier>, key: Key },
    Paste(PasteMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PasteMode {
    EachCycle,
    Once,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Modifier {
    Control,
    Shift,
    Alt,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key {
    Character(char),
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

pub(crate) fn parse(command: &str) -> Result<Vec<InputAction>, String> {
    let mut actions = Vec::new();
    let mut text = String::new();
    let mut characters = command.chars().peekable();

    while let Some(character) = characters.next() {
        if character != '!' {
            text.push(character);
        } else if characters.peek() == Some(&'!') {
            characters.next();
            text.push('!');
        } else {
            let mut token = String::new();
            let mut closed = false;
            for token_character in characters.by_ref() {
                if token_character == '!' {
                    closed = true;
                    break;
                }
                token.push(token_character);
            }
            if !closed {
                return Err("unterminated input token".to_owned());
            }
            flush_text(&mut actions, &mut text);
            actions.push(parse_token(&token)?);
        }
    }

    flush_text(&mut actions, &mut text);
    Ok(actions)
}

fn flush_text(actions: &mut Vec<InputAction>, text: &mut String) {
    if !text.is_empty() {
        actions.push(InputAction::Text(std::mem::take(text)));
    }
}

fn parse_token(token: &str) -> Result<InputAction, String> {
    match token {
        "PASTE" => return Ok(InputAction::Paste(PasteMode::EachCycle)),
        "PASTE_ONCE" => return Ok(InputAction::Paste(PasteMode::Once)),
        _ => {}
    }
    let mut parts = token.split('+');
    if parts.clone().any(str::is_empty) {
        return Err(format!("invalid input token: !{token}!"));
    }
    let key = parse_key(parts.next_back().expect("split always contains a key"))
        .ok_or_else(|| format!("unsupported input token: !{token}!"))?;
    let mut modifiers = Vec::new();
    for part in parts {
        let modifier = match part {
            "CTRL" => Modifier::Control,
            "SHIFT" => Modifier::Shift,
            "ALT" => Modifier::Alt,
            "COMMAND" | "CMD" => Modifier::Command,
            _ => return Err(format!("unsupported input token: !{token}!")),
        };
        if modifiers.contains(&modifier) {
            return Err(format!("duplicate modifier in input token: !{token}!"));
        }
        modifiers.push(modifier);
    }
    if modifiers.is_empty() {
        Ok(InputAction::Key(key))
    } else {
        Ok(InputAction::Chord { modifiers, key })
    }
}

pub(crate) fn resolve_paste(
    actions: &[InputAction],
    mode: PasteMode,
    read_clipboard: impl FnOnce() -> Result<String, String>,
) -> Result<Cow<'_, [InputAction]>, String> {
    if !actions.contains(&InputAction::Paste(mode)) {
        return Ok(Cow::Borrowed(actions));
    }
    let text = read_clipboard()?;
    if text.contains('\0') {
        return Err("clipboard text contains a NUL character".to_owned());
    }
    Ok(Cow::Owned(
        actions
            .iter()
            .map(|action| match action {
                InputAction::Paste(paste_mode) if *paste_mode == mode => {
                    InputAction::Text(text.clone())
                }
                other => other.clone(),
            })
            .collect(),
    ))
}

fn parse_key(value: &str) -> Option<Key> {
    match value {
        "ENTER" => Some(Key::Enter),
        "TAB" => Some(Key::Tab),
        "ESC" => Some(Key::Escape),
        "BACKSPACE" => Some(Key::Backspace),
        "DELETE" => Some(Key::Delete),
        "UP" => Some(Key::Up),
        "DOWN" => Some(Key::Down),
        "LEFT" => Some(Key::Left),
        "RIGHT" => Some(Key::Right),
        "HOME" => Some(Key::Home),
        "END" => Some(Key::End),
        "PAGEUP" => Some(Key::PageUp),
        "PAGEDOWN" => Some(Key::PageDown),
        value if value.len() == 1 => value
            .chars()
            .next()
            .map(|character| Key::Character(character.to_ascii_lowercase())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{InputAction, Key, Modifier, PasteMode, parse};

    #[test]
    fn parses_mixed_actions() {
        assert_eq!(
            parse("/你好!ENTER!!CTRL+C!").expect("valid input actions"),
            vec![
                InputAction::Text("/你好".to_owned()),
                InputAction::Key(Key::Enter),
                InputAction::Chord {
                    modifiers: vec![Modifier::Control],
                    key: Key::Character('c'),
                },
            ]
        );
    }

    #[test]
    fn parses_only_explicit_tokens_as_key_actions() {
        assert_eq!(
            parse("!T!!PASTE!123!ENTER!").unwrap(),
            vec![
                InputAction::Key(Key::Character('t')),
                InputAction::Paste(PasteMode::EachCycle),
                InputAction::Text("123".to_owned()),
                InputAction::Key(Key::Enter),
            ]
        );
        assert_eq!(
            parse("Hello!! world 你好!T!123").unwrap(),
            vec![
                InputAction::Text("Hello! world 你好".to_owned()),
                InputAction::Key(Key::Character('t')),
                InputAction::Text("123".to_owned()),
            ]
        );
    }

    #[test]
    fn resolves_paste_without_converting_regular_text_to_keys() {
        let actions = parse("!T!!PASTE!123!ENTER!").unwrap();
        assert_eq!(
            super::resolve_paste(&actions, PasteMode::EachCycle, || Ok("!ENTER!测试".into()))
                .unwrap()
                .as_ref(),
            vec![
                InputAction::Key(Key::Character('t')),
                InputAction::Text("!ENTER!测试".into()),
                InputAction::Text("123".into()),
                InputAction::Key(Key::Enter),
            ]
        );
    }

    #[test]
    fn escapes_literal_exclamation_marks() {
        assert_eq!(
            parse("!!").expect("escaped exclamation mark"),
            vec![InputAction::Text("!".to_owned())]
        );
    }

    #[test]
    fn parses_adjacent_tokens_without_treating_the_boundary_as_an_escape() {
        assert_eq!(
            parse("!CTRL+V!!ENTER!").expect("adjacent tokens"),
            vec![
                InputAction::Chord {
                    modifiers: vec![Modifier::Control],
                    key: Key::Character('v'),
                },
                InputAction::Key(Key::Enter),
            ]
        );
    }

    #[test]
    fn token_character_keys_are_physical_base_keys() {
        assert_eq!(
            parse("!CTRL+V!!CTRL+SHIFT+Z!").expect("physical token keys"),
            vec![
                InputAction::Chord {
                    modifiers: vec![Modifier::Control],
                    key: Key::Character('v'),
                },
                InputAction::Chord {
                    modifiers: vec![Modifier::Control, Modifier::Shift],
                    key: Key::Character('z'),
                },
            ]
        );
    }

    #[test]
    fn accepts_command_modifier_aliases() {
        assert_eq!(
            parse("!COMMAND+V!!CMD+C!").expect("macOS command tokens"),
            vec![
                InputAction::Chord {
                    modifiers: vec![Modifier::Command],
                    key: Key::Character('v'),
                },
                InputAction::Chord {
                    modifiers: vec![Modifier::Command],
                    key: Key::Character('c'),
                },
            ]
        );
    }

    #[test]
    fn rejects_invalid_tokens() {
        assert!(parse("!UNKNOWN!").is_err());
        assert!(parse("!ENTER").is_err());
        assert!(parse("!CTRL+CTRL+V!").is_err());
        assert!(parse("!CMD+COMMAND+V!").is_err());
    }

    #[test]
    fn paste_is_resolved_once_as_literal_text_without_reparsing() {
        let actions = parse("!PASTE!!ENTER!!PASTE!").unwrap();
        let resolved = super::resolve_paste(&actions, PasteMode::EachCycle, || {
            Ok("!CTRL+C!\n你好".into())
        })
        .unwrap();
        assert_eq!(
            resolved.as_ref(),
            vec![
                InputAction::Text("!CTRL+C!\n你好".into()),
                InputAction::Key(Key::Enter),
                InputAction::Text("!CTRL+C!\n你好".into()),
            ]
        );
    }

    #[test]
    fn paste_errors_are_propagated_without_reading_when_absent() {
        assert!(
            super::resolve_paste(
                &[InputAction::Paste(PasteMode::EachCycle)],
                PasteMode::EachCycle,
                || Err("clipboard error".into())
            )
            .is_err()
        );
        assert!(
            super::resolve_paste(
                &[InputAction::Paste(PasteMode::EachCycle)],
                PasteMode::EachCycle,
                || Ok("a\0b".into())
            )
            .is_err()
        );
        assert!(
            super::resolve_paste(
                &[InputAction::Key(Key::Enter)],
                PasteMode::EachCycle,
                || panic!("unexpected clipboard access")
            )
            .is_ok()
        );
    }

    #[test]
    fn clipboard_is_read_once_per_cycle_and_not_at_all_without_paste() {
        let actions = parse("!PASTE! + !PASTE!").unwrap();
        let mut reads = 0;
        for expected in ["first", "second"] {
            let result = super::resolve_paste(&actions, PasteMode::EachCycle, || {
                reads += 1;
                Ok(expected.to_owned())
            })
            .unwrap();
            assert_eq!(
                result.as_ref(),
                &[
                    InputAction::Text(expected.into()),
                    InputAction::Text(" + ".into()),
                    InputAction::Text(expected.into()),
                ]
            );
        }
        assert_eq!(reads, 2);
        let no_paste = parse("hello!ENTER!").unwrap();
        let result = super::resolve_paste(&no_paste, PasteMode::EachCycle, || {
            panic!("clipboard must not be read")
        })
        .unwrap();
        assert!(matches!(result, std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn resolves_startup_snapshot_once_and_refreshes_cycle_pastes() {
        let actions = parse("!PASTE_ONCE!|!PASTE!|!PASTE_ONCE!|!PASTE!").unwrap();
        let mut reads = 0;
        let startup = super::resolve_paste(&actions, PasteMode::Once, || {
            reads += 1;
            Ok("!ENTER!".into())
        })
        .unwrap();
        assert_eq!(reads, 1);
        for expected in ["first", "second"] {
            let cycle = super::resolve_paste(&startup, PasteMode::EachCycle, || {
                reads += 1;
                Ok(expected.to_owned())
            })
            .unwrap();
            assert_eq!(
                cycle.as_ref(),
                &[
                    InputAction::Text("!ENTER!".into()),
                    InputAction::Text("|".into()),
                    InputAction::Text(expected.into()),
                    InputAction::Text("|".into()),
                    InputAction::Text("!ENTER!".into()),
                    InputAction::Text("|".into()),
                    InputAction::Text(expected.into()),
                ]
            );
        }
        assert_eq!(reads, 3);
    }

    #[test]
    fn startup_snapshot_errors_propagate_and_other_modes_are_not_read() {
        let actions = parse("!PASTE_ONCE!").unwrap();
        assert_eq!(
            super::resolve_paste(&actions, PasteMode::Once, || Err("clipboard error".into()))
                .unwrap_err(),
            "clipboard error"
        );
        assert!(super::resolve_paste(&actions, PasteMode::Once, || Ok("a\0b".into())).is_err());
        assert!(matches!(
            super::resolve_paste(&actions, PasteMode::EachCycle, || panic!("unexpected read"))
                .unwrap(),
            std::borrow::Cow::Borrowed(_)
        ));
        let dynamic = parse("!PASTE!").unwrap();
        assert!(matches!(
            super::resolve_paste(&dynamic, PasteMode::Once, || panic!("unexpected read")).unwrap(),
            std::borrow::Cow::Borrowed(_)
        ));
        assert!(parse("!paste_once!").is_err());
    }
}
