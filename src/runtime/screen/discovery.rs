#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenSessionInfo {
    pub id: String,
    pub name: String,
}

pub fn find_screen_sessions(session_match: &str) -> Result<Vec<ScreenSessionInfo>, String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::process::Command;

        let output = Command::new("screen")
            .arg("-list")
            .env("LC_ALL", "C")
            .output()
            .map_err(|error| format!("failed to list GNU screen sessions: {error}"))?;

        if !output.status.success() {
            let no_sessions = output.status.code() == Some(1)
                && output.stderr.is_empty()
                && output.stdout.starts_with(b"No Sockets found in ");
            if no_sessions {
                return Ok(Vec::new());
            }
            return Err(format!(
                "failed to list GNU screen sessions ({})",
                super::command_failure_detail(&output)
            ));
        }

        Ok(parse_screen_sessions(
            &String::from_utf8_lossy(&output.stdout),
            session_match,
        ))
    }

    #[cfg(target_os = "windows")]
    {
        let _ = session_match;
        Err("screen session mode is not supported on Windows".to_owned())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn parse_screen_sessions(output: &str, session_match: &str) -> Vec<ScreenSessionInfo> {
    output
        .lines()
        .filter_map(|line| {
            let (session, _) = line.trim_start().rsplit_once('\t')?;
            let (pid, name) = session.split_once('.')?;
            (!pid.is_empty()
                && pid.bytes().all(|byte| byte.is_ascii_digit())
                && !name.is_empty()
                && name.contains(session_match))
            .then(|| ScreenSessionInfo {
                id: session.to_owned(),
                name: name.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ScreenSessionInfo, parse_screen_sessions};

    #[test]
    fn parses_matching_screen_session_names() {
        let sessions = parse_screen_sessions(
            "There are screens on:\n\t123.fb_first\t(Detached)\n\t456.other\t(Attached)\n",
            "fb_",
        );
        assert_eq!(
            sessions,
            vec![ScreenSessionInfo {
                id: "123.fb_first".into(),
                name: "fb_first".into()
            }]
        );
    }

    #[test]
    fn preserves_unique_ids_and_spaces_in_session_names() {
        let sessions = parse_screen_sessions(
            "There are screens on:\n\t123.fb one\t(Detached)\n\t456.fb one\t(Attached)\n\t789.fb two\t(Detached)\n",
            "fb one",
        );
        assert_eq!(
            sessions,
            vec![
                ScreenSessionInfo {
                    id: "123.fb one".into(),
                    name: "fb one".into()
                },
                ScreenSessionInfo {
                    id: "456.fb one".into(),
                    name: "fb one".into()
                },
            ]
        );
    }
}
