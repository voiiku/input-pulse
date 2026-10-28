use crate::runtime::input_actions::{InputAction, Key, Modifier};
use std::borrow::Cow;
use std::time::Duration;

pub(crate) fn validate_actions(actions: &[InputAction]) -> Result<(), String> {
    for action in actions {
        match action {
            InputAction::Chord { modifiers, key } => {
                screen_chord_sequence(modifiers, *key)?;
            }
            InputAction::Text(text) if text.contains('\0') => {
                return Err("GNU screen input cannot contain NUL".to_owned());
            }
            InputAction::Key(Key::Character('\0')) => {
                return Err("GNU screen input cannot contain NUL".to_owned());
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn render_actions(
    actions: &[InputAction],
    input_delay: Duration,
) -> Result<Vec<Cow<'_, str>>, String> {
    let mut rendered = Vec::new();

    for action in actions {
        match action {
            InputAction::Text(text) => {
                if text.contains('\0') {
                    return Err("GNU screen input cannot contain NUL".to_owned());
                }
                if input_delay.is_zero() {
                    rendered.push(Cow::Borrowed(text.as_str()));
                } else {
                    rendered.extend(text.char_indices().map(|(index, character)| {
                        Cow::Borrowed(&text[index..index + character.len_utf8()])
                    }));
                }
            }
            InputAction::Key(Key::Character(character)) => {
                if *character == '\0' {
                    return Err("GNU screen input cannot contain NUL".to_owned());
                }
                rendered.push(Cow::Owned(character.to_string()));
            }
            InputAction::Key(key) => rendered.push(Cow::Borrowed(screen_key_sequence(*key))),
            InputAction::Chord { modifiers, key } => {
                rendered.push(Cow::Owned(
                    screen_chord_sequence(modifiers, *key)?.to_string(),
                ));
            }
            InputAction::Paste(_) => return Err("clipboard input was not resolved".to_owned()),
        }
    }
    Ok(rendered)
}

fn screen_key_sequence(key: Key) -> &'static str {
    match key {
        Key::Character(_) => unreachable!("character keys are rendered separately"),
        Key::Enter => "\r",
        Key::Tab => "\t",
        Key::Escape => "\x1b",
        Key::Backspace => "\x08",
        Key::Delete => "\x1b[3~",
        Key::Up => "\x1b[A",
        Key::Down => "\x1b[B",
        Key::Left => "\x1b[D",
        Key::Right => "\x1b[C",
        Key::Home => "\x1b[H",
        Key::End => "\x1b[F",
        Key::PageUp => "\x1b[5~",
        Key::PageDown => "\x1b[6~",
    }
}

fn screen_chord_sequence(modifiers: &[Modifier], key: Key) -> Result<char, String> {
    if modifiers != [Modifier::Control] {
        return Err(
            "GNU screen tokens support only CTRL combined with an ASCII control key".to_owned(),
        );
    }

    let Key::Character(character) = key else {
        return Err(
            "GNU screen CTRL tokens require an ASCII character key, such as !CTRL+C!".to_owned(),
        );
    };

    let control = match character {
        '@'..='_' => (character as u8) & 0x1f,
        'a'..='z' => (character.to_ascii_uppercase() as u8) & 0x1f,
        '?' => 0x7f,
        _ => {
            return Err(
                "GNU screen CTRL tokens require an ASCII letter or control character".to_owned(),
            );
        }
    };

    if control == 0 {
        return Err("GNU screen cannot pass a NUL byte through a process argument".to_owned());
    }

    Ok(char::from(control))
}

#[cfg(test)]
mod tests {
    use crate::runtime::input_actions::{InputAction, Key};
    use std::time::Duration;

    fn render_command(command: &str) -> Result<String, String> {
        let actions = crate::runtime::input_actions::parse(command)?;
        super::validate_actions(&actions)?;
        super::render_actions(&actions, Duration::ZERO).map(|actions| actions.concat())
    }

    #[test]
    fn renders_screen_control_tokens() {
        assert_eq!(
            render_command("status!ENTER!!CTRL+C!").expect("screen command"),
            "status\r\u{3}"
        );
    }

    #[test]
    fn renders_control_a_and_rejects_ctrl_nul() {
        assert_eq!(render_command("!CTRL+A!").unwrap(), "\x01");
        assert!(render_command("!CTRL+@!").is_err());
    }

    #[test]
    fn renders_standard_terminal_key_sequences() {
        assert_eq!(
            render_command("!UP!!DELETE!").expect("screen command"),
            "\u{1b}[A\u{1b}[3~"
        );
    }

    #[test]
    fn rejects_non_terminal_chords() {
        assert!(render_command("!ALT+X!").is_err());
        assert!(render_command("!SHIFT+ENTER!").is_err());
    }

    #[test]
    fn renders_resolved_clipboard_literally() {
        let parsed = crate::runtime::input_actions::parse("!PASTE!!ENTER!").unwrap();
        let actions = crate::runtime::input_actions::resolve_paste(
            &parsed,
            crate::runtime::input_actions::PasteMode::EachCycle,
            || Ok("!CTRL+C! hello".into()),
        )
        .unwrap();
        assert_eq!(
            super::render_actions(&actions, Duration::ZERO)
                .unwrap()
                .concat(),
            "!CTRL+C! hello\r"
        );
    }

    #[test]
    fn renders_explicit_key_paste_and_literal_text_in_order() {
        let parsed = crate::runtime::input_actions::parse("!T!!PASTE!123!ENTER!").unwrap();
        let actions = crate::runtime::input_actions::resolve_paste(
            &parsed,
            crate::runtime::input_actions::PasteMode::EachCycle,
            || Ok("!CTRL+C!".into()),
        )
        .unwrap();
        assert_eq!(
            super::render_actions(&actions, Duration::ZERO)
                .unwrap()
                .concat(),
            "t!CTRL+C!123\r"
        );
    }

    #[test]
    fn text_characters_and_special_keys_are_separate_timing_steps() {
        let actions = crate::runtime::input_actions::parse("你a!UP!!CTRL+A!!ENTER!").unwrap();
        assert_eq!(
            super::render_actions(&actions, Duration::from_millis(1)).unwrap(),
            vec!["你", "a", "\x1b[A", "\x01", "\r"]
        );
    }

    #[test]
    fn text_steps_borrow_the_original_utf8_data() {
        let actions = crate::runtime::input_actions::parse("你a!UP!").unwrap();
        let steps = super::render_actions(&actions, Duration::from_millis(1)).unwrap();
        assert!(
            steps
                .iter()
                .all(|step| matches!(step, std::borrow::Cow::Borrowed(_)))
        );
        assert_eq!(steps, vec!["你", "a", "\x1b[A"]);
    }

    #[test]
    fn refuses_unresolved_paste_and_nul_before_dispatch() {
        use crate::runtime::input_actions::PasteMode;
        for actions in [
            vec![InputAction::Paste(PasteMode::EachCycle)],
            vec![InputAction::Paste(PasteMode::Once)],
            vec![InputAction::Text("before\0after".into())],
            vec![InputAction::Key(Key::Character('\0'))],
        ] {
            for delay in [Duration::ZERO, Duration::from_millis(1)] {
                assert!(super::render_actions(&actions, delay).is_err());
            }
        }
    }

    #[test]
    fn zero_delay_keeps_long_utf8_text_in_one_borrowed_step() {
        let text = "你a".repeat(100_000);
        let actions = [
            InputAction::Text(text.clone()),
            InputAction::Key(Key::Enter),
        ];
        let steps = super::render_actions(&actions, Duration::ZERO).unwrap();
        assert_eq!(steps.len(), 2);
        assert!(matches!(steps[0], std::borrow::Cow::Borrowed(_)));
        assert_eq!(steps.concat(), format!("{text}\r"));
    }
}
