use crate::runtime::{
    clipboard, input_actions,
    screen::{self, ScreenSessionInfo},
    window::{self, WindowInfo},
};
use crate::{RunEvent, StopToken};
use std::time::Duration;

pub fn execute_for_windows(
    windows: &[WindowInfo],
    actions: &[input_actions::InputAction],
    simulated_key_delay: Duration,
    stop: &StopToken,
    on_event: &mut impl FnMut(RunEvent),
) -> Result<(), String> {
    if stop.is_stop_requested() {
        return Ok(());
    }
    let actions = input_actions::resolve_paste(
        actions,
        input_actions::PasteMode::EachCycle,
        clipboard::read_text,
    )?;
    execute_targets(
        windows.iter().map(|window| (window.title.as_str(), window)),
        stop,
        on_event,
        |window| window::send_command(window, &actions, simulated_key_delay),
    )
}

pub fn execute_for_screen_sessions(
    sessions: &[ScreenSessionInfo],
    actions: &[input_actions::InputAction],
    input_delay: Duration,
    stop: &StopToken,
    on_event: &mut impl FnMut(RunEvent),
) -> Result<(), String> {
    if stop.is_stop_requested() {
        return Ok(());
    }
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
        stop,
        on_event,
        |session| screen::send_command(session, &input, input_delay),
    )
}

fn execute_targets<'a, T: 'a>(
    targets: impl IntoIterator<Item = (&'a str, &'a T)>,
    stop: &StopToken,
    on_event: &mut impl FnMut(RunEvent),
    mut send: impl FnMut(&T) -> Result<(), String>,
) -> Result<(), String> {
    let mut failures = Vec::new();
    for (name, target) in targets {
        if stop.is_stop_requested() {
            break;
        }
        on_event(RunEvent::Target {
            name: name.to_owned(),
        });
        if stop.is_stop_requested() {
            break;
        }
        on_event(RunEvent::CommandStarted);
        if stop.is_stop_requested() {
            break;
        }
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
    use crate::StopToken;

    #[test]
    fn finishes_current_target_but_skips_remaining_targets() {
        let stop = StopToken::new();
        let mut events = Vec::new();
        let mut sent = Vec::new();
        execute_targets(
            [("one", &1), ("two", &2)],
            &stop,
            &mut |event| events.push(event),
            |target| {
                sent.push(*target);
                stop.request_stop();
                // The rest of the current target's input still executes.
                sent.push(*target);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(sent, [1, 1]);
        assert_eq!(
            events,
            [
                RunEvent::Target { name: "one".into() },
                RunEvent::CommandStarted,
                RunEvent::CommandComplete,
            ]
        );
    }

    #[test]
    fn stop_does_not_hide_delivery_errors() {
        let stop = StopToken::new();
        let error = execute_targets([("one", &()), ("two", &())], &stop, &mut |_| {}, |_| {
            stop.request_stop();
            Err("delivery failed".into())
        })
        .unwrap_err();
        assert_eq!(error, "one: delivery failed");
    }

    #[test]
    fn stop_before_dispatch_or_from_callback_sends_nothing() {
        for boundary in 0..=2 {
            let stop = StopToken::new();
            if boundary == 0 {
                stop.request_stop();
            }
            let mut events = Vec::new();
            execute_targets(
                [("one", &())],
                &stop,
                &mut |event| {
                    events.push(event);
                    if events.len() == boundary {
                        stop.request_stop();
                    }
                },
                |_| panic!("input must not be sent after a stop"),
            )
            .unwrap();
            assert_eq!(events.len(), boundary);
        }
    }

    #[test]
    fn reports_each_target_and_keeps_failures_visible() {
        let targets = [("one", false), ("two", true), ("three", false)];
        let mut events = Vec::new();
        let error = execute_targets(
            targets.iter().map(|(name, fail)| (*name, fail)),
            &crate::StopToken::new(),
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
