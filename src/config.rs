use crate::runtime::{
    input_actions::{self, InputAction, Modifier},
    screen,
};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetMode {
    /// Find visible windows whose title contains `InputPulseConfig::target_match`.
    Window,
    /// Find GNU screen sessions whose name contains `InputPulseConfig::target_match`.
    ScreenSession,
}

/// Configures an input-pulse run.
///
/// The runner treats `!`-wrapped tokens as key actions.
/// Unwrapped characters are treated as Unicode text.
///
/// X11 Window targets deliver text through best-effort simulated keys.
/// `!PASTE!` inserts the current clipboard text once per matching input cycle.
/// `!PASTE_ONCE!` captures clipboard text at the start of each run and reuses it.
///
/// Input settings must be supplied explicitly; this type does not implement
/// [`Default`]. CLI process options default to `false` in [`Self::new`] and
/// are not acted on by the library's [`crate::run`] function.
///
/// ```compile_fail
/// use input_pulse::InputPulseConfig;
/// let _ = InputPulseConfig::default();
/// ```
///
/// ```compile_fail
/// use input_pulse::{InputPulseConfig, TargetMode};
/// let _ = InputPulseConfig::new(TargetMode::Window, "Minecraft", "hello", 60);
/// ```
#[derive(Debug, Clone)]
pub struct InputPulseConfig {
    /// Selects whether targets are windows or GNU screen sessions.
    pub target_mode: TargetMode,
    /// Case-sensitive substring used to select targets.
    pub target_match: String,
    /// Content sent to each matching target.
    pub command: String,
    /// Delay between completed input cycles.
    pub execution_interval_seconds: u64,
    /// Delay after each key action and text character, in seconds.
    pub input_delay_seconds: f64,
    /// Request CLI help; [`crate::run`] does not interpret this field. The CLI
    /// can use a non-runnable help-only configuration when no target is supplied.
    pub help: bool,
    /// Request CLI background execution; [`crate::run`] does not interpret this field.
    pub background: bool,
}

impl InputPulseConfig {
    /// Constructs and validates a configuration with explicitly supplied input settings.
    /// The CLI-only `help` and `background` flags are initialized to `false`.
    ///
    /// This does not check platform prerequisites. Since fields are public,
    /// call [`Self::validate`] again after changing them; [`crate::run`] also
    /// validates before running.
    ///
    /// ```
    /// use input_pulse::{InputPulseConfig, TargetMode};
    ///
    /// let config = InputPulseConfig::new(
    ///     TargetMode::ScreenSession,
    ///     "server",
    ///     "status!ENTER!",
    ///     60,
    ///     0.05,
    /// )?;
    /// assert_eq!(config.input_delay_seconds, 0.05);
    /// assert!(!config.help && !config.background);
    /// # Ok::<(), String>(())
    /// ```
    pub fn new(
        target_mode: TargetMode,
        target_match: impl Into<String>,
        command: impl Into<String>,
        execution_interval_seconds: u64,
        input_delay_seconds: f64,
    ) -> Result<Self, String> {
        let config = Self {
            target_mode,
            target_match: target_match.into(),
            command: command.into(),
            execution_interval_seconds,
            input_delay_seconds,
            help: false,
            background: false,
        };
        validate_config(&config)?;
        Ok(config)
    }

    /// Validates the target and command before starting a run.
    pub fn validate(&self) -> Result<(), String> {
        validate_config(self).map(|_| ())
    }
}

pub(crate) fn validate_config(config: &InputPulseConfig) -> Result<Vec<InputAction>, String> {
    if config.target_match.trim().is_empty() {
        return Err("target match must not be empty".to_owned());
    }
    if config.command.trim().is_empty() {
        return Err("command must not be empty or whitespace-only".to_owned());
    }
    if Duration::try_from_secs_f64(config.input_delay_seconds).is_err() {
        return Err("input delay must be a non-negative number of seconds within the supported duration range".to_owned());
    }

    let actions = input_actions::parse(&config.command)?;
    if config.target_mode == TargetMode::ScreenSession {
        screen::validate_actions(&actions)?;
    } else if cfg!(not(target_os = "macos"))
        && actions.iter().any(|action| {
            matches!(action, InputAction::Chord { modifiers, .. } if modifiers.contains(&Modifier::Command))
        })
    {
        return Err("the COMMAND modifier is only supported by macOS window targets".to_owned());
    }
    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::{InputPulseConfig, TargetMode, validate_config};

    fn config(target_mode: TargetMode, target_match: &str, command: &str) -> InputPulseConfig {
        InputPulseConfig {
            target_mode,
            target_match: target_match.to_owned(),
            command: command.to_owned(),
            execution_interval_seconds: 60,
            input_delay_seconds: 0.05,
            help: false,
            background: false,
        }
    }

    #[test]
    fn new_constructs_and_validates_required_configuration() {
        let config = InputPulseConfig::new(
            TargetMode::Window,
            "Minecraft",
            "!T!/home fishing!ENTER!",
            90,
            0.12,
        )
        .unwrap();
        assert_eq!(config.target_mode, TargetMode::Window);
        assert_eq!(config.target_match, "Minecraft");
        assert_eq!(config.command, "!T!/home fishing!ENTER!");
        assert_eq!(config.execution_interval_seconds, 90);
        assert_eq!(config.input_delay_seconds, 0.12);
        assert!(!config.help);
        assert!(!config.background);
        assert!(InputPulseConfig::new(TargetMode::Window, "", "hello", 60, 0.05).is_err());
        assert!(
            InputPulseConfig::new(TargetMode::Window, "target", "!UNKNOWN!", 60, 0.05).is_err()
        );
        assert!(
            InputPulseConfig::new(TargetMode::ScreenSession, "target", "!ALT+X!", 60, 0.05)
                .is_err()
        );
        assert!(InputPulseConfig::new(TargetMode::Window, "target", "hello", 60, -1.0).is_err());
    }

    #[test]
    fn public_field_changes_remain_subject_to_validation() {
        let mut config =
            InputPulseConfig::new(TargetMode::Window, "target", "hello", 60, 0.05).unwrap();
        config.input_delay_seconds = f64::NAN;
        assert!(config.validate().is_err());
    }

    #[test]
    fn cli_flags_do_not_change_library_validation() {
        let mut config =
            InputPulseConfig::new(TargetMode::Window, "target", "hello", 60, 0.05).unwrap();
        config.help = true;
        config.background = true;
        assert!(config.validate().is_ok());
        config.command.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn simulated_key_target_accepts_unicode_text_fallback() {
        let config = config(TargetMode::Window, "target", "/帮助");
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn screen_session_mode_accepts_utf8_commands() {
        let config = config(TargetMode::ScreenSession, "target", "/帮助");
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn paste_token_validates_for_both_targets_without_reading_clipboard() {
        for target_mode in [TargetMode::Window, TargetMode::ScreenSession] {
            let config = config(target_mode, "target", "!PASTE_ONCE!!PASTE!!ENTER!");
            assert!(validate_config(&config).is_ok());
        }
    }

    #[test]
    fn rejects_empty_commands() {
        for command in ["", "   "] {
            let config = config(TargetMode::Window, "target", command);
            assert!(validate_config(&config).is_err());
        }
    }

    #[test]
    fn rejects_empty_target_matches() {
        assert!(validate_config(&config(TargetMode::Window, "", "hello")).is_err());
    }

    #[test]
    fn rejects_key_delays_that_cannot_be_represented() {
        for delay in [-1.0, f64::NAN, f64::INFINITY, 1e300] {
            let mut config = config(TargetMode::Window, "target", "!ENTER!");
            config.input_delay_seconds = delay;
            assert!(validate_config(&config).is_err(), "delay: {delay}");
        }
    }

    #[test]
    fn accepts_commands_with_special_characters() {
        let command = "echo $(id); [keepalive]";
        let config = config(TargetMode::ScreenSession, "target", command);
        assert!(validate_config(&config).is_ok());
        assert_eq!(config.command, command);
    }

    #[test]
    fn screen_session_mode_rejects_non_terminal_chords() {
        let config = config(TargetMode::ScreenSession, "target", "!ALT+X!");
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn window_mode_accepts_best_effort_modifier_chords() {
        let config = config(TargetMode::Window, "target", "!CTRL+V!!ENTER!");
        assert!(validate_config(&config).is_ok());
    }
}
