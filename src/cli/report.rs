use chrono::Local;
use input_pulse::RunEvent;
use std::io::{self, Write};
use std::time::Duration;

pub fn show_event(event: RunEvent) {
    match event {
        RunEvent::CommandSafetyWarning { characters } => {
            println!(
                "Warning: command contains potentially unsafe characters {characters:?}. You accept all responsibility for its effects."
            );
        }

        RunEvent::ClipboardSafetyWarning => {
            println!(
                "Warning: clipboard text is sent to every matching target. Newlines and control characters may execute commands."
            );
        }
        RunEvent::Started => {
            println!("==================================================");
            println!("               Input Pulse");
            println!("==================================================");
            println!("\nSearching for matching targets...");
        }
        RunEvent::NoTargetFound => {
            println!("\nNo matching target found; scanning will continue...")
        }
        RunEvent::MatchingTargets {
            count,
            interval_seconds,
        } => {
            println!("Matching targets: {count}");
            println!("Interval: {interval_seconds}s");
            println!("\n==================================================");
        }
        RunEvent::Target { name } => println!("Controlling target: {name}"),
        RunEvent::CommandStarted => println!("[{}] Sending input", Local::now().format("%H:%M:%S")),
        RunEvent::CommandComplete => {
            println!("  Input dispatched (application acceptance is not verified)\n")
        }
        RunEvent::ExecutionCount {
            count,
            interval_seconds,
        } => {
            let next = chrono::Duration::from_std(Duration::from_secs(interval_seconds))
                .ok()
                .and_then(|duration| Local::now().checked_add_signed(duration));
            if let Some(next) = next {
                println!(
                    "Executed {count} times | Next: {}\n",
                    next.format("%H:%M:%S")
                );
            } else {
                println!("Executed {count} times | Next: outside supported date range\n");
            }
        }
        RunEvent::Waiting {
            elapsed_seconds,
            interval_seconds,
        } => {
            print!("\rWaiting... {elapsed_seconds}s / {interval_seconds}s elapsed");
            io::stdout()
                .flush()
                .expect("writing progress to stdout succeeds");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RunEvent, show_event};

    #[test]
    fn reports_large_intervals_without_panicking() {
        for interval_seconds in [0, 10_000_000_000_000, u64::MAX] {
            show_event(RunEvent::ExecutionCount {
                count: 1,
                interval_seconds,
            });
        }
    }
}
