#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("input-pulse supports Windows, macOS, and Linux only");

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod config;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod runtime;

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use config::validate_config;
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub use config::{InputPulseConfig, TargetMode};
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub use runtime::platform::check_platform_support;

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

/// Runs input-pulse cycles and reports progress to `on_event`.
///
/// Call this from a dedicated thread when the host must continue doing work.
/// The library does not write to stdout or stderr. Hosts can localize and
/// display events, forward them to a logger, or ignore them as appropriate.
/// The `help` and `background` fields are CLI options and are not interpreted.
///
/// ```no_run
/// use input_pulse::{InputPulseConfig, TargetMode};
///
/// let config = InputPulseConfig::new(
///     TargetMode::ScreenSession, "server", "status!ENTER!", 60, 0.05,
/// )?;
/// input_pulse::run(config, |_| {})?;
/// # Ok::<(), String>(())
/// ```
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub fn run(config: InputPulseConfig, mut on_event: impl FnMut(RunEvent)) -> Result<(), String> {
    // Check platform prerequisites first so hosts receive actionable setup
    // errors even when the remaining configuration is incomplete.
    check_platform_support(config.target_mode)?;
    let actions = validate_config(&config)?;

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
    let actions = runtime::input_actions::resolve_paste(
        &actions,
        runtime::input_actions::PasteMode::Once,
        runtime::clipboard::read_text,
    )?;
    on_event(RunEvent::Started);
    runtime::runner::run_cycles(&config, &actions, &mut on_event)?;
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
    use super::command_warning_characters;

    #[test]
    fn identifies_command_warning_characters() {
        assert_eq!(
            command_warning_characters("echo $(id);"),
            vec!['$', '(', ')', ';']
        );
    }
}
