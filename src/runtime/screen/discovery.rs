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

        let output = String::from_utf8(output.stdout)
            .map_err(|error| format!("GNU screen session list was not valid UTF-8: {error}"))?;
        parse_screen_sessions(&output, session_match)
    }

    #[cfg(target_os = "windows")]
    {
        let _ = session_match;
        Err("screen session mode is not supported on Windows".to_owned())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn parse_screen_sessions(
    output: &str,
    session_match: &str,
) -> Result<Vec<ScreenSessionInfo>, String> {
    let mut recognized = false;
    let sessions = output
        .lines()
        .filter_map(|line| {
            let (session, status) = line.trim_start().rsplit_once('\t')?;
            let status = status.trim();
            if !status.starts_with('(') || !status.ends_with(')') || is_timestamp(status) {
                return None;
            }
            let session = match session.rsplit_once('\t') {
                Some((session, field)) if is_timestamp(field) => session,
                _ => session,
            };
            let (pid, name) = session.split_once('.')?;
            if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) || name.is_empty() {
                return None;
            }
            recognized = true;
            name.contains(session_match).then(|| ScreenSessionInfo {
                id: session.to_owned(),
                name: name.to_owned(),
            })
        })
        .collect();
    if recognized {
        Ok(sessions)
    } else {
        Err(format!(
            "unrecognized GNU screen session list: {}",
            output.trim()
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn is_timestamp(field: &str) -> bool {
    let Some(value) = field
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return false;
    };
    // The final field before the status can also be part of a tabbed name.
    value.bytes().any(|byte| byte == b' ' || byte == b'T')
        && value.as_bytes().windows(3).any(|window| {
            window[0].is_ascii_digit() && window[1] == b':' && window[2].is_ascii_digit()
        })
}

#[cfg(test)]
mod tests {
    use super::{ScreenSessionInfo, parse_screen_sessions};

    #[test]
    fn parses_matching_screen_session_names() {
        let sessions = parse_screen_sessions(
            "There are screens on:\n\t123.fb_first\t(Detached)\n\t456.other\t(Attached)\n",
            "fb_",
        )
        .unwrap();
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
        )
        .unwrap();
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

    #[test]
    fn strips_timestamps_from_ids_names_and_match_checks() {
        let output = concat!(
            "There are screens on:\n",
            "\t123.fb one\t(10/29/2026 09:11:12 PM)\t(Detached)\n",
            "\t456.fb one\t(Thu Oct 29 21:11:12 2026)\t(Attached)\n",
            "\t789.fb two\t(2026-10-29T21:11:12)\t(Detached)\n",
            "3 Sockets in /run/screen/S-user.\n",
        );
        assert_eq!(
            parse_screen_sessions(output, "fb one").unwrap(),
            vec![
                ScreenSessionInfo {
                    id: "123.fb one".into(),
                    name: "fb one".into(),
                },
                ScreenSessionInfo {
                    id: "456.fb one".into(),
                    name: "fb one".into(),
                },
            ]
        );
        assert!(parse_screen_sessions(output, "2026").unwrap().is_empty());
    }

    #[test]
    fn preserves_tabs_and_parentheses_in_session_names() {
        let output = concat!(
            "\t123.fb\t(test)\t(Detached)\n",
            "\t456.fb\t(test)\t(10/29/2026 09:11:12 PM)\t(Attached)\n",
        );
        assert_eq!(
            parse_screen_sessions(output, "fb\t(test)").unwrap(),
            vec![
                ScreenSessionInfo {
                    id: "123.fb\t(test)".into(),
                    name: "fb\t(test)".into(),
                },
                ScreenSessionInfo {
                    id: "456.fb\t(test)".into(),
                    name: "fb\t(test)".into(),
                },
            ]
        );
    }

    #[test]
    fn ignores_lines_without_a_valid_session_id() {
        let output = concat!(
            "There are screens on:\n",
            "\tinvalid.fb\t(10/29/2026 09:11:12 PM)\t(Detached)\n",
            "\t.fb\t(Detached)\n",
            "\t123.\t(Detached)\n",
            "\t123fb\t(Detached)\n",
            "\t456.fb\ttruncated status\n",
            "\t789.fb\t(10/29/2026 09:11:12 PM)\n",
            "No Sockets found in /run/screen/S-user.\n",
        );
        assert!(parse_screen_sessions(output, "").is_err());
    }
}
