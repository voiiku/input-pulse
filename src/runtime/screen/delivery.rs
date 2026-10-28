use super::discovery::ScreenSessionInfo;
use std::borrow::Cow;
use std::time::Duration;

// GNU screen's command socket has a small fixed-size message buffer.
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
const SCREEN_STUFF_CHUNK_BYTES: usize = 512;

pub fn send_command(
    session: &ScreenSessionInfo,
    input: &[Cow<'_, str>],
    delay: Duration,
) -> Result<(), String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        dispatch_input(input, delay, |chunk, index| {
            let output = std::process::Command::new("screen")
                .args(["-S", &session.id, "-X", "stuff", chunk])
                .output()
                .map_err(|error| format!("failed to send input step {index} to GNU screen session {}: {error}; earlier steps may already have been delivered", session.id))?;
            if !output.status.success() {
                return Err(format!(
                    "failed to send input step {index} to GNU screen session {} ({}); earlier steps may already have been delivered",
                    session.id,
                    super::command_failure_detail(&output)
                ));
            }
            Ok(())
        })
    }

    #[cfg(target_os = "windows")]
    {
        let _ = (session, input, delay);
        Err("screen session mode is not supported on Windows".to_owned())
    }
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn dispatch_input(
    input: &[Cow<'_, str>],
    delay: Duration,
    mut send: impl FnMut(&str, usize) -> Result<(), String>,
) -> Result<(), String> {
    if delay.is_zero() {
        let input = match input {
            [step] => Cow::Borrowed(step.as_ref()),
            _ => Cow::Owned(input.iter().map(Cow::as_ref).collect::<String>()),
        };
        for (index, chunk) in screen_chunks(&input).enumerate() {
            send(chunk, index + 1)?;
        }
    } else {
        for (index, step) in input.iter().enumerate() {
            send(step.as_ref(), index + 1)?;
            std::thread::sleep(delay);
        }
    }
    Ok(())
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn screen_chunks(mut input: &str) -> impl Iterator<Item = &str> {
    std::iter::from_fn(move || {
        if input.is_empty() {
            return None;
        }

        let mut end = input.len().min(SCREEN_STUFF_CHUNK_BYTES);
        while !input.is_char_boundary(end) {
            end -= 1;
        }
        if end < input.len() {
            // Keep the longest supported terminal escape sequences together.
            if let Some(position) = input.as_bytes()[end.saturating_sub(3)..end]
                .iter()
                .rposition(|byte| *byte == b'\x1b')
            {
                end = end.saturating_sub(3) + position;
            }
        }
        let (chunk, rest) = input.split_at(end);
        input = rest;
        Some(chunk)
    })
}

#[cfg(test)]
mod tests {
    use super::{SCREEN_STUFF_CHUNK_BYTES, dispatch_input, screen_chunks};
    use std::time::{Duration, Instant};

    #[test]
    fn chunks_long_inputs_without_splitting_utf8() {
        let input = format!("{}你好!{}", "a".repeat(511), "b".repeat(1300));
        let chunks: Vec<_> = screen_chunks(&input).collect();
        assert!(chunks.len() > 2);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.len() <= SCREEN_STUFF_CHUNK_BYTES)
        );
        assert_eq!(chunks.concat(), input);
    }

    #[test]
    fn keeps_navigation_escape_sequences_together() {
        let input = format!("{}\x1b[3~more", "a".repeat(511));
        let chunks: Vec<_> = screen_chunks(&input).collect();
        assert_eq!(chunks[0], "a".repeat(511));
        assert_eq!(chunks[1], "\x1b[3~more");
    }

    #[test]
    fn nonzero_delay_waits_after_each_complete_step() {
        let steps = vec!["你".into(), "x".into(), "\x1b[A".into()];
        let mut sent = Vec::new();
        let start = Instant::now();
        dispatch_input(&steps, Duration::from_millis(10), |step, index| {
            sent.push((step.to_owned(), index));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            sent,
            vec![("你".into(), 1), ("x".into(), 2), ("\x1b[A".into(), 3)]
        );
        assert!(start.elapsed() >= Duration::from_millis(30));
    }

    #[test]
    fn zero_delay_batches_without_losing_utf8_or_key_sequences() {
        let steps = vec!["你".into(); 300];
        let mut chunks = Vec::new();
        dispatch_input(&steps, Duration::ZERO, |chunk, _| {
            chunks.push(chunk.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(chunks.concat(), steps.concat());
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.len() <= SCREEN_STUFF_CHUNK_BYTES)
        );
    }

    #[test]
    fn zero_delay_chunks_a_single_text_step_without_copying() {
        let text = "你a".repeat(300);
        let steps = vec![std::borrow::Cow::Borrowed(text.as_str())];
        let start = text.as_ptr() as usize;
        let end = start + text.len();
        let mut chunks = Vec::new();
        dispatch_input(&steps, Duration::ZERO, |chunk, _| {
            let chunk_start = chunk.as_ptr() as usize;
            assert!(chunk_start >= start && chunk_start + chunk.len() <= end);
            chunks.push(chunk.to_owned());
            Ok(())
        })
        .unwrap();
        assert!(chunks.len() > 1);
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn delivery_error_stops_later_steps() {
        let steps = vec!["a".into(), "b".into(), "c".into()];
        let mut sent = Vec::new();
        let error = dispatch_input(&steps, Duration::from_nanos(1), |step, index| {
            sent.push(step.to_owned());
            if index == 2 {
                Err("failed".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, "failed");
        assert_eq!(sent, vec!["a", "b"]);
    }
}
