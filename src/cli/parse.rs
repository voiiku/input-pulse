use super::USAGE;
use input_pulse::{InputPulseConfig, TargetMode};
pub(super) fn parse_cli_arguments(
    arguments: impl IntoIterator<Item = String>,
) -> Result<InputPulseConfig, String> {
    (|| {
        const DEFAULT_KEY_DELAY_SECONDS: f64 = 0.05;

        let mut arguments = arguments.into_iter().peekable();
        let mut help = false;
        let mut background = false;
        while let Some(argument) = arguments.peek() {
            match argument.as_str() {
                "-h" | "--help" => help = true,
                "-b" | "--background" => background = true,
                option if option.starts_with('-') => {
                    return Err(format!("unsupported option: {option}"));
                }
                _ => break,
            }
            arguments.next();
        }

        let Some(target) = arguments.next() else {
            if help {
                return Ok(InputPulseConfig {
                    target_mode: TargetMode::Window,
                    target_match: String::new(),
                    command: String::new(),
                    execution_interval_seconds: 0,
                    input_delay_seconds: DEFAULT_KEY_DELAY_SECONDS,
                    help,
                    background,
                });
            }
            return Err("missing required positional arguments".to_owned());
        };
        let target_mode = match target.as_str() {
            "window" => TargetMode::Window,
            "screen" => TargetMode::ScreenSession,
            _ => return Err(format!("unsupported target: {target}")),
        };
        let target_match = arguments
            .next()
            .ok_or_else(|| "missing required positional arguments".to_owned())?;
        let command = arguments
            .next()
            .ok_or_else(|| "missing required positional arguments".to_owned())?;
        let interval = arguments
            .next()
            .ok_or_else(|| "missing required interval-seconds".to_owned())?
            .parse()
            .map_err(|_| "interval must be an unsigned integer number of seconds".to_owned())?;
        let input_delay = arguments
            .next()
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| "key delay must be a non-negative number of seconds".to_owned())
            })
            .transpose()?
            .unwrap_or(DEFAULT_KEY_DELAY_SECONDS);
        if let Some(extra) = arguments.next() {
            return Err(format!("unexpected argument: {extra}"));
        }
        let mut config =
            InputPulseConfig::new(target_mode, target_match, command, interval, input_delay)?;
        config.help = help;
        config.background = background;
        Ok(config)
    })()
    .map_err(|error| format!("{error}\n\n{USAGE}"))
}

#[cfg(test)]
mod tests {
    use super::parse_cli_arguments;
    use input_pulse::TargetMode;

    fn config(args: &[&str]) -> Result<input_pulse::InputPulseConfig, String> {
        parse_cli_arguments(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn help_only_is_non_runnable_and_bypasses_missing_positionals() {
        for args in [&["--help"][..], &["-h"][..], &["-b", "--help"][..]] {
            let parsed = config(args).unwrap();
            assert!(parsed.help);
            assert!(parsed.validate().is_err());
            assert_eq!(parsed.background, args.contains(&"-b"));
        }
        assert!(config(&[]).unwrap_err().contains("missing required"));
    }

    #[test]
    fn parses_required_interval_and_optional_key_delay() {
        let parsed = config(&[
            "--background",
            "window",
            "Minecraft",
            "!T!/123!ENTER!",
            "45",
        ])
        .unwrap();
        assert_eq!(parsed.target_mode, TargetMode::Window);
        assert_eq!(parsed.execution_interval_seconds, 45);
        assert_eq!(parsed.input_delay_seconds, 0.05);
        assert!(parsed.background);
        assert!(!parsed.help);
        let parsed = config(&["-b", "screen", "worker", "status!ENTER!", "23", "0.12"]).unwrap();
        assert_eq!(parsed.target_mode, TargetMode::ScreenSession);
        assert_eq!(parsed.execution_interval_seconds, 23);
        assert_eq!(parsed.input_delay_seconds, 0.12);
        assert_eq!(
            config(&["screen", "worker", "status", "0"])
                .unwrap()
                .execution_interval_seconds,
            0
        );
    }

    #[test]
    fn help_with_arguments_is_validated() {
        let parsed = config(&["-h", "window", "Minecraft", "hello", "60", "0"]).unwrap();
        assert!(parsed.help);
        assert!(parsed.validate().is_ok());
        assert!(
            config(&["--help", "window", "editor", "!UNKNOWN!", "60"])
                .unwrap_err()
                .contains("unsupported input token")
        );
        assert!(
            config(&["-h", "screen", "worker", "status", "invalid"])
                .unwrap_err()
                .contains("interval")
        );
        assert!(
            config(&["--help", "window"])
                .unwrap_err()
                .contains("missing required")
        );
    }

    #[test]
    fn validates_intervals_delays_and_option_order() {
        for args in [
            &["screen", "worker", "status", "later"][..],
            &["screen", "worker", "status", "-1"][..],
            &["screen", "worker", "status", "60", "not-a-number"][..],
            &["window", "editor", "text", "60", "-1"][..],
            &["window", "editor", "text", "60", "1e300"][..],
            &["window", "editor", "text", "--background"][..],
            &["window", "editor", "text", "60", "0.1", "extra"][..],
            &["--unknown", "window", "editor", "text"][..],
        ] {
            let error = config(args).unwrap_err();
            assert_eq!(error.matches("Usage:").count(), 1, "{error}");
        }
    }

    #[test]
    fn interval_is_required_even_with_options() {
        for args in [
            &["window", "editor", "text"][..],
            &["--background", "window", "editor", "text"][..],
            &["--help", "window", "editor", "text"][..],
        ] {
            let error = config(args).unwrap_err();
            assert!(
                error.contains("missing required interval-seconds"),
                "{error}"
            );
            assert_eq!(error.matches("Usage:").count(), 1, "{error}");
        }
    }

    #[test]
    fn positional_commands_can_look_like_options() {
        let parsed = config(&["window", "Editor", "--background", "60"]).unwrap();
        assert_eq!(parsed.command, "--background");
        assert!(!parsed.background && !parsed.help);
        let parsed = config(&["window", "Editor", "--help", "60"]).unwrap();
        assert_eq!(parsed.command, "--help");
        assert!(!parsed.help);
        assert_eq!(
            config(&["window", "Editor", "--", "60"]).unwrap().command,
            "--"
        );
        assert_eq!(
            config(&["window", "Minecraft", "/home fishing", "60"])
                .unwrap()
                .command,
            "/home fishing"
        );
    }

    #[test]
    fn rejects_end_of_options_marker() {
        for args in [
            &["--", "window", "Editor", "hello"][..],
            &["--help", "--", "window", "Editor", "hello"][..],
        ] {
            assert!(config(args).unwrap_err().contains("unsupported option: --"));
        }
    }
}
