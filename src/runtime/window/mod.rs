mod discovery;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(test, target_os = "windows"))]
mod windows;

pub(crate) use discovery::{WindowInfo, find_windows};

#[cfg(target_os = "linux")]
pub(crate) use linux::send_command;
#[cfg(target_os = "macos")]
pub(crate) use macos::send_command;
#[cfg(target_os = "windows")]
pub(crate) use windows::send_command;

#[cfg(target_os = "linux")]
fn x11_error(error: impl std::fmt::Display) -> String {
    format!("X11 operation failed: {error}")
}

#[cfg(any(test, target_os = "windows", target_os = "macos"))]
fn dispatch_actions(
    actions: &[super::input_actions::InputAction],
    delay: std::time::Duration,
    mut send_character: impl FnMut(char) -> Result<(), String>,
    mut send_key: impl FnMut(
        super::input_actions::Key,
        &[super::input_actions::Modifier],
    ) -> Result<(), String>,
) -> Result<(), String> {
    use super::input_actions::InputAction;

    for action in actions {
        match action {
            InputAction::Text(text) => {
                for character in text.chars() {
                    send_character(character)?;
                    std::thread::sleep(delay);
                }
            }
            InputAction::Key(key) => {
                send_key(*key, &[])?;
                std::thread::sleep(delay);
            }
            InputAction::Chord { key, modifiers } => {
                send_key(*key, modifiers)?;
                std::thread::sleep(delay);
            }
            InputAction::Paste(_) => return Err("clipboard input was not resolved".to_owned()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        super::input_actions::{InputAction, Key, Modifier},
        dispatch_actions,
    };
    use std::{
        cell::RefCell,
        time::{Duration, Instant},
    };

    #[test]
    fn dispatches_text_and_keys_in_order_and_stops_on_error() {
        let actions = [
            InputAction::Text("A你".into()),
            InputAction::Chord {
                key: Key::Character('v'),
                modifiers: vec![Modifier::Control],
            },
            InputAction::Key(Key::Enter),
        ];
        let sent = RefCell::new(Vec::new());
        dispatch_actions(
            &actions,
            Duration::ZERO,
            |character| {
                sent.borrow_mut().push(format!("text:{character}"));
                Ok(())
            },
            |key, modifiers| {
                sent.borrow_mut().push(format!("{key:?}:{modifiers:?}"));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            sent.into_inner(),
            ["text:A", "text:你", "Character('v'):[Control]", "Enter:[]"]
        );

        let calls = RefCell::new(0);
        let error = dispatch_actions(
            &actions,
            Duration::ZERO,
            |_| {
                *calls.borrow_mut() += 1;
                Err("failed".into())
            },
            |_, _| {
                *calls.borrow_mut() += 1;
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error, "failed");
        assert_eq!(calls.into_inner(), 1);
        assert!(
            dispatch_actions(
                &[InputAction::Paste(
                    super::super::input_actions::PasteMode::EachCycle
                )],
                Duration::ZERO,
                |_| Ok(()),
                |_, _| Ok(())
            )
            .is_err()
        );
    }

    #[test]
    fn waits_after_each_character_and_key() {
        let actions = [InputAction::Text("ab".into()), InputAction::Key(Key::Enter)];
        let start = Instant::now();
        dispatch_actions(
            &actions,
            Duration::from_millis(10),
            |_| Ok(()),
            |_, _| Ok(()),
        )
        .unwrap();
        assert!(start.elapsed() >= Duration::from_millis(30));
    }
}
