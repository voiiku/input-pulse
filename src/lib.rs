#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("input-pulse supports Windows, macOS, and Linux only");

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod config;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod runtime;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod stop;

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use config::validate_config;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub use config::{InputPulseConfig, TargetMode};
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub use runtime::platform::check_platform_support;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use stop::StopToken;

/// Describes observable progress during an input-pulse run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub enum RunEvent {
    /// The command contains characters that can be interpreted by shells or
    /// terminal sessions.
    CommandSafetyWarning {
        characters: Vec<char>,
    },
    /// Clipboard content will be sent to each matching target, including any
    /// newlines and control characters it contains.
    ClipboardSafetyWarning,
    Started,
    NoTargetFound,
    MatchingTargets {
        count: usize,
        interval_seconds: u64,
    },
    Target {
        name: String,
    },
    CommandStarted,
    CommandComplete,
    ExecutionCount {
        count: u32,
        interval_seconds: u64,
    },
    Waiting {
        elapsed_seconds: u64,
        interval_seconds: u64,
    },
}

/// An input worker that owns its configuration, thread and stop state.
///
/// Events run on the worker thread; the library writes nothing to stdout or
/// stderr. The configuration's `help` and `background` fields are CLI-only.
/// Dropping the object requests a stop without joining. Call [`Self::stop`] or
/// [`Self::wait`] explicitly to confirm termination and collect runtime errors.
///
/// ```no_run
/// use input_pulse::{InputPulse, InputPulseConfig, TargetMode};
///
/// let config = InputPulseConfig::new(
///     TargetMode::ScreenSession, "server", "status!ENTER!", 60, 0.05,
/// )?;
/// let mut pulse = InputPulse::new(config)?;
/// pulse.start(|_| {})?;
/// // When the host wants to stop:
/// pulse.stop()?;
/// # Ok::<(), String>(())
/// ```
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub struct InputPulse {
    config: InputPulseConfig,
    stop: StopToken,
    worker: Option<std::thread::JoinHandle<Result<(), String>>>,
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
impl InputPulse {
    /// Validates and stores a configuration without starting a thread or
    /// checking platform prerequisites.
    pub fn new(config: InputPulseConfig) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            config,
            stop: StopToken::new(),
            worker: None,
        })
    }

    /// Checks platform prerequisites and starts a dedicated worker thread.
    ///
    /// Returning `Ok` confirms thread creation, not successful input delivery.
    /// `Started` reports that worker initialization has completed. Callbacks
    /// must return promptly and must not wait for the host to stop the worker.
    /// Call [`Self::stop`] or [`Self::wait`] to collect a previous worker's
    /// result before starting again, even if [`Self::is_running`] is false.
    pub fn start(&mut self, on_event: impl FnMut(RunEvent) + Send + 'static) -> Result<(), String> {
        if self.worker.is_some() {
            return Err(
                "collect the previous worker with stop() or wait() before starting again".into(),
            );
        }
        check_platform_support(self.config.target_mode)?;
        let config = self.config.clone();
        let stop = StopToken::new();
        let worker_stop = stop.clone();
        let worker = std::thread::Builder::new()
            .name("input-pulse".into())
            .spawn(move || run(config, &worker_stop, on_event))
            .map_err(|error| format!("failed to start input worker: {error}"))?;
        self.stop = stop;
        self.worker = Some(worker);
        Ok(())
    }

    /// Reports thread liveness, not whether matching targets exist or input
    /// delivery is succeeding. Use stop or wait to retrieve an exited worker's error.
    pub fn is_running(&self) -> bool {
        self.worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
    }

    /// Requests a stop and joins the worker, returning any runtime error or panic.
    ///
    /// Inter-cycle waits wake immediately. The current target's complete input,
    /// including key delays, finishes before stopping; subsequent targets are
    /// skipped. A request racing with dispatch may allow that target to finish.
    /// Blocking calls and callbacks cannot be interrupted, so shutdown has no
    /// fixed time bound. Do not call this on an async executor thread.
    ///
    /// With no worker this returns `Ok(())`. After collecting the result, the
    /// object can be started again with fresh stop state.
    pub fn stop(&mut self) -> Result<(), String> {
        self.stop.request_stop();
        self.wait()
    }

    /// Joins the worker without requesting a stop and returns its result.
    ///
    /// Normally blocks until an error occurs; intended for foreground CLI use.
    /// Errors are returned once, when the worker is collected. A panic is
    /// converted to an error. With no worker this returns `Ok(())`.
    pub fn wait(&mut self) -> Result<(), String> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        worker.join().map_err(|payload| {
            let detail = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic payload");
            format!("input worker panicked: {detail}")
        })?
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
impl Drop for InputPulse {
    /// Requests a stop without blocking. Call stop or wait explicitly to join
    /// the thread and retrieve its result before dropping the object.
    fn drop(&mut self) {
        self.stop.request_stop();
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn run(
    config: InputPulseConfig,
    stop: &StopToken,
    mut on_event: impl FnMut(RunEvent),
) -> Result<(), String> {
    if stop.is_stop_requested() {
        return Ok(());
    }
    let actions = validate_config(&config)?;
    if stop.is_stop_requested() {
        return Ok(());
    }

    let warning_characters = command_warning_characters(&config.command);
    if !warning_characters.is_empty() {
        on_event(RunEvent::CommandSafetyWarning {
            characters: warning_characters,
        });
    }
    if actions
        .iter()
        .any(|action| matches!(action, runtime::input_actions::InputAction::Paste(_)))
    {
        on_event(RunEvent::ClipboardSafetyWarning);
    }
    if stop.is_stop_requested() {
        return Ok(());
    }
    let actions = runtime::input_actions::resolve_paste(
        &actions,
        runtime::input_actions::PasteMode::Once,
        runtime::clipboard::read_text,
    )?;
    if stop.is_stop_requested() {
        return Ok(());
    }
    on_event(RunEvent::Started);
    runtime::runner::run_cycles(&config, &actions, stop, &mut on_event)?;
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn command_warning_characters(command: &str) -> Vec<char> {
    command
        .chars()
        .filter(|character| {
            matches!(
                character,
                '\n' | '\r'
                    | '|'
                    | '&'
                    | ';'
                    | '<'
                    | '>'
                    | '$'
                    | '('
                    | ')'
                    | '`'
                    | '\\'
                    | '"'
                    | '\''
                    | '{'
                    | '}'
                    | '*'
                    | '?'
            )
        })
        .collect()
}

#[cfg(all(
    test,
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
mod tests {
    use super::*;
    use std::{sync::mpsc, thread, time::Duration};

    fn pulse() -> InputPulse {
        InputPulse::new(InputPulseConfig::new(TargetMode::Window, "unused", "x", 60, 0.0).unwrap())
            .unwrap()
    }

    #[test]
    fn constructor_revalidates_mutated_configuration() {
        let mut config = InputPulseConfig::new(TargetMode::Window, "unused", "x", 60, 0.0).unwrap();
        config.command.clear();
        assert!(InputPulse::new(config).is_err());
    }

    #[test]
    fn stop_joins_worker_and_preserves_its_error() {
        let mut pulse = pulse();
        let stop = pulse.stop.clone();
        pulse.worker = Some(thread::spawn(move || {
            assert!(stop.wait(Duration::from_secs(60)));
            Err("delivery failed during shutdown".into())
        }));
        assert!(pulse.is_running());
        assert!(pulse.start(|_| {}).is_err());
        assert_eq!(pulse.stop().unwrap_err(), "delivery failed during shutdown");
        assert!(!pulse.is_running());
        pulse.stop().unwrap();
        pulse.wait().unwrap();
    }

    #[test]
    fn exited_worker_must_be_collected_before_starting_again() {
        let mut pulse = pulse();
        let (send, receive) = mpsc::channel();
        pulse.worker = Some(thread::spawn(move || {
            send.send(()).unwrap();
            Err("runtime failed".into())
        }));
        receive.recv_timeout(Duration::from_secs(2)).unwrap();
        while pulse.is_running() {
            thread::yield_now();
        }
        assert!(pulse.start(|_| {}).is_err());
        assert_eq!(pulse.wait().unwrap_err(), "runtime failed");
        assert!(pulse.worker.is_none());
    }

    #[test]
    fn worker_panic_is_returned_as_an_error() {
        let mut pulse = pulse();
        pulse.worker = Some(thread::spawn(|| panic!("callback failed")));
        assert_eq!(
            pulse.wait().unwrap_err(),
            "input worker panicked: callback failed"
        );
        assert!(!pulse.is_running());
        pulse.stop().unwrap();
    }

    #[test]
    fn dropping_object_requests_stop() {
        let mut pulse = pulse();
        let stop = pulse.stop.clone();
        let (send, receive) = mpsc::channel();
        pulse.worker = Some(thread::spawn(move || {
            let stopped = stop.wait(Duration::from_secs(60));
            send.send(stopped).unwrap();
            Ok(())
        }));
        drop(pulse);
        assert!(receive.recv_timeout(Duration::from_secs(2)).unwrap());
    }

    #[test]
    fn pre_stopped_run_skips_initialization_and_events() {
        let config =
            super::InputPulseConfig::new(super::TargetMode::Window, "unused", "x", 60, 0.0)
                .unwrap();
        let stop = super::StopToken::new();
        stop.request_stop();
        super::run(config, &stop, |_| panic!("already stopped")).unwrap();
    }

    #[test]
    fn identifies_command_warning_characters() {
        assert_eq!(
            command_warning_characters("echo $(id);"),
            vec!['$', '(', ')', ';']
        );
    }
}
