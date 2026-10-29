mod parse;
mod report;

use input_pulse::{InputPulse, InputPulseConfig, TargetMode};
use parse::parse_cli_arguments;
use std::process::{Command, Stdio};

const USAGE: &str = "Usage:
  input-pulse [options] <target> <target-match> <command> <interval-seconds> [key-delay-seconds]

Interval:
  interval-seconds  Wait between input cycles.

Targets:
  window  Send input to every visible window whose title contains <target-match>.
  screen  Send terminal input and supported tokens to matching GNU screen sessions.

Input delay:
  key-delay-seconds  Wait after each key or text character (default: 0.05).

General options:
  -b, --background  Run in the background and print its process ID.
  -h, --help        Show this help text and exit.

Options must precede <target>.
Check https://github.com/voiiku/input-pulse for more information.";

pub fn run() -> Result<(), String> {
    let config = parse_cli_arguments(std::env::args().skip(1))?;
    if config.help {
        println!("{USAGE}");
        return Ok(());
    }
    if config.background {
        input_pulse::check_platform_support(config.target_mode)?;
        start_background(&config)?;
    } else {
        let mut pulse = InputPulse::new(config)?;
        pulse.start(report::show_event)?;
        pulse.wait()?;
    }
    Ok(())
}

fn start_background(config: &InputPulseConfig) -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to locate the current executable: {error}"))?;
    let mut command = Command::new(executable);
    command
        .args(arguments_for_child(config))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        command.process_group(0);
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    let child = command
        .spawn()
        .map_err(|error| format!("failed to start the background process: {error}"))?;
    println!(
        "Input Pulse started in the background with PID {}.",
        child.id()
    );
    Ok(())
}

fn arguments_for_child(config: &InputPulseConfig) -> Vec<String> {
    let target = match config.target_mode {
        TargetMode::Window => "window",
        TargetMode::ScreenSession => "screen",
    };
    vec![
        target.to_owned(),
        config.target_match.clone(),
        config.command.clone(),
        config.execution_interval_seconds.to_string(),
        config.input_delay_seconds.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{arguments_for_child, parse_cli_arguments};
    use input_pulse::{InputPulseConfig, TargetMode};

    #[test]
    fn background_child_arguments_reconstruct_window_configuration() {
        let mut config =
            InputPulseConfig::new(TargetMode::Window, "Editor", "--help!ENTER!", 60, 0.12).unwrap();
        config.background = true;
        let arguments = arguments_for_child(&config);
        assert_eq!(
            arguments,
            vec!["window", "Editor", "--help!ENTER!", "60", "0.12"]
        );
        let child = parse_cli_arguments(arguments).unwrap();
        assert!(!child.help && !child.background);
        assert_eq!(child.target_mode, config.target_mode);
        assert_eq!(child.target_match, config.target_match);
        assert_eq!(child.command, config.command);
        assert_eq!(
            child.execution_interval_seconds,
            config.execution_interval_seconds
        );
        assert_eq!(child.input_delay_seconds, config.input_delay_seconds);
    }

    #[test]
    fn background_screen_arguments_preserve_input_delay() {
        let mut config = InputPulseConfig::new(
            TargetMode::ScreenSession,
            "worker",
            "status!ENTER!",
            90,
            0.12,
        )
        .unwrap();
        config.background = true;
        let arguments = arguments_for_child(&config);
        assert_eq!(
            arguments,
            vec!["screen", "worker", "status!ENTER!", "90", "0.12"]
        );
        let child = parse_cli_arguments(arguments).unwrap();
        assert!(!child.help && !child.background);
        assert_eq!(child.command, config.command);
        assert_eq!(
            child.execution_interval_seconds,
            config.execution_interval_seconds
        );
        assert_eq!(child.input_delay_seconds, config.input_delay_seconds);
    }
}
