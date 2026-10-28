use crate::RunEvent;
use crate::runtime::{
    clipboard, input_actions,
    screen::{self, ScreenSessionInfo},
    window::{self, WindowInfo},
};
use std::time::Duration;

pub fn execute_for_windows(
    windows: &[WindowInfo],
    actions: &[input_actions::InputAction],
    simulated_key_delay: Duration,
    on_event: &mut impl FnMut(RunEvent),
) -> Result<(), String> {
    let actions = input_actions::resolve_paste(
        actions,
        input_actions::PasteMode::EachCycle,
        clipboard::read_text,
    )?;
    execute_targets(
        windows.iter().map(|window| (window.title.as_str(), window)),
        on_event,
        |window| window::send_command(window, &actions, simulated_key_delay),
    )
}

pub fn execute_for_screen_sessions(
    sessions: &[ScreenSessionInfo],
    actions: &[input_actions::InputAction],
    input_delay: Duration,
    on_event: &mut impl FnMut(RunEvent),
) -> Result<(), String> {
    let actions = input_actions::resolve_paste(
        actions,
        input_actions::PasteMode::EachCycle,
        clipboard::read_text,
    )?;
    let input = screen::render_actions(&actions, input_delay)?;
    execute_targets(
        sessions
            .iter()
            .map(|session| (session.name.as_str(), session)),
        on_event,
        |session| screen::send_command(session, &input, input_delay),
    )
}

fn execute_targets<'a, T: 'a>(
    targets: impl IntoIterator<Item = (&'a str, &'a T)>,
    on_event: &mut impl FnMut(RunEvent),
    mut send: impl FnMut(&T) -> Result<(), String>,
) -> Result<(), String> {
    let mut failures = Vec::new();
    for (name, target) in targets {
        on_event(RunEvent::Target {
            name: name.to_owned(),
        });
        on_event(RunEvent::CommandStarted);
        if let Err(error) = send(target) {
            failures.push(format!("{name}: {error}"));
        } else {
            on_event(RunEvent::CommandComplete);
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::{RunEvent, execute_targets};

    #[test]
    fn reports_each_target_and_keeps_failures_visible() {
        let targets = [("one", false), ("two", true), ("three", false)];
        let mut events = Vec::new();
        let error = execute_targets(
            targets.iter().map(|(name, fail)| (*name, fail)),
            &mut |event| events.push(event),
            |fail| if *fail { Err("failed".into()) } else { Ok(()) },
        )
        .unwrap_err();
        assert_eq!(error, "two: failed");
        assert_eq!(
            events,
            vec![
                RunEvent::Target { name: "one".into() },
                RunEvent::CommandStarted,
                RunEvent::CommandComplete,
                RunEvent::Target { name: "two".into() },
                RunEvent::CommandStarted,
                RunEvent::Target {
                    name: "three".into()
                },
                RunEvent::CommandStarted,
                RunEvent::CommandComplete,
            ]
        );
    }
}
