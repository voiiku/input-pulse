use crate::{InputPulseConfig, RunEvent, TargetMode};
use std::thread;
use std::time::{Duration, Instant};

use super::{executor, input_actions::InputAction, screen, window};

const WAIT_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) fn run_cycles(
    config: &InputPulseConfig,
    actions: &[InputAction],
    on_event: &mut impl FnMut(RunEvent),
) -> Result<(), String> {
    let mut count = 0;
    loop {
        let executed = match config.target_mode {
            TargetMode::Window => {
                let windows = window::find_windows(&config.target_match)?;
                if windows.is_empty() {
                    false
                } else {
                    report_matches(windows.len(), config.execution_interval_seconds, on_event);
                    executor::execute_for_windows(
                        &windows,
                        actions,
                        Duration::from_secs_f64(config.input_delay_seconds),
                        on_event,
                    )?;
                    true
                }
            }
            TargetMode::ScreenSession => {
                let sessions = screen::find_screen_sessions(&config.target_match)?;
                if sessions.is_empty() {
                    false
                } else {
                    report_matches(sessions.len(), config.execution_interval_seconds, on_event);
                    executor::execute_for_screen_sessions(
                        &sessions,
                        actions,
                        Duration::from_secs_f64(config.input_delay_seconds),
                        on_event,
                    )?;
                    true
                }
            }
        };
        if executed {
            count += 1;
            on_event(RunEvent::ExecutionCount {
                count,
                interval_seconds: config.execution_interval_seconds,
            });
        } else {
            on_event(RunEvent::NoTargetFound);
        }
        wait_for_next_cycle(config.execution_interval_seconds, on_event);
    }
}

fn report_matches(count: usize, interval_seconds: u64, on_event: &mut impl FnMut(RunEvent)) {
    on_event(RunEvent::MatchingTargets {
        count,
        interval_seconds,
    });
}

fn wait_for_next_cycle(interval_seconds: u64, on_event: &mut impl FnMut(RunEvent)) {
    let wait_started = Instant::now();
    let execution_interval = Duration::from_secs(interval_seconds);
    loop {
        let elapsed = wait_started.elapsed();
        if elapsed >= execution_interval {
            return;
        }
        on_event(RunEvent::Waiting {
            elapsed_seconds: elapsed.as_secs(),
            interval_seconds,
        });
        thread::sleep(
            execution_interval
                .saturating_sub(elapsed)
                .min(WAIT_POLL_INTERVAL),
        );
    }
}
