use crate::{InputPulseConfig, RunEvent, StopToken, TargetMode};
use std::time::{Duration, Instant};

use super::{executor, input_actions::InputAction, screen, window};

const WAIT_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) fn run_cycles(
    config: &InputPulseConfig,
    actions: &[InputAction],
    stop: &StopToken,
    on_event: &mut impl FnMut(RunEvent),
) -> Result<(), String> {
    let mut count = 0;
    loop {
        if stop.is_stop_requested() {
            return Ok(());
        }
        let executed = match config.target_mode {
            TargetMode::Window => {
                let windows = window::find_windows(&config.target_match)?;
                if stop.is_stop_requested() {
                    return Ok(());
                }
                if windows.is_empty() {
                    false
                } else {
                    report_matches(windows.len(), config.execution_interval_seconds, on_event);
                    executor::execute_for_windows(
                        &windows,
                        actions,
                        Duration::from_secs_f64(config.input_delay_seconds),
                        stop,
                        on_event,
                    )?;
                    true
                }
            }
            TargetMode::ScreenSession => {
                let sessions = screen::find_screen_sessions(&config.target_match)?;
                if stop.is_stop_requested() {
                    return Ok(());
                }
                if sessions.is_empty() {
                    false
                } else {
                    report_matches(sessions.len(), config.execution_interval_seconds, on_event);
                    executor::execute_for_screen_sessions(
                        &sessions,
                        actions,
                        Duration::from_secs_f64(config.input_delay_seconds),
                        stop,
                        on_event,
                    )?;
                    true
                }
            }
        };
        if stop.is_stop_requested() {
            return Ok(());
        }
        if executed {
            count += 1;
            on_event(RunEvent::ExecutionCount {
                count,
                interval_seconds: config.execution_interval_seconds,
            });
        } else {
            on_event(RunEvent::NoTargetFound);
        }
        wait_for_next_cycle(config.execution_interval_seconds, stop, on_event);
    }
}

fn report_matches(count: usize, interval_seconds: u64, on_event: &mut impl FnMut(RunEvent)) {
    on_event(RunEvent::MatchingTargets {
        count,
        interval_seconds,
    });
}

fn wait_for_next_cycle(
    interval_seconds: u64,
    stop: &StopToken,
    on_event: &mut impl FnMut(RunEvent),
) {
    let wait_started = Instant::now();
    let execution_interval = Duration::from_secs(interval_seconds);
    loop {
        if stop.is_stop_requested() {
            return;
        }
        let elapsed = wait_started.elapsed();
        if elapsed >= execution_interval {
            return;
        }
        on_event(RunEvent::Waiting {
            elapsed_seconds: elapsed.as_secs(),
            interval_seconds,
        });
        if stop.wait(
            execution_interval
                .saturating_sub(elapsed)
                .min(WAIT_POLL_INTERVAL),
        ) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_from_waiting_callback_returns_without_sleeping() {
        let stop = StopToken::new();
        let started = Instant::now();
        let mut events = Vec::new();
        wait_for_next_cycle(60, &stop, &mut |event| {
            events.push(event);
            stop.request_stop();
        });
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            RunEvent::Waiting {
                interval_seconds: 60,
                ..
            }
        ));
        assert!(started.elapsed() < WAIT_POLL_INTERVAL);
    }

    #[test]
    fn pre_stopped_and_zero_interval_waits_emit_no_events() {
        let stop = StopToken::new();
        wait_for_next_cycle(0, &stop, &mut |_| panic!("no wait expected"));
        stop.request_stop();
        wait_for_next_cycle(60, &stop, &mut |_| panic!("already stopped"));
    }

    #[test]
    fn pre_stopped_cycle_does_not_discover_targets() {
        let stop = StopToken::new();
        stop.request_stop();
        let config = InputPulseConfig::new(TargetMode::Window, "unused", "x", 60, 0.0).unwrap();
        run_cycles(&config, &[], &stop, &mut |_| panic!("already stopped")).unwrap();
    }
}
