use input_pulse::{InputPulseConfig, RunEvent, TargetMode};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let target = args.next().ok_or_else(|| {
        "usage: cargo run --example screen -- <screen-session-substring>".to_owned()
    })?;
    if args.next().is_some() {
        return Err("expected exactly one screen-session substring".to_owned());
    }

    let config = InputPulseConfig::new(
        TargetMode::ScreenSession,
        target,
        "echo input-pulse-example!ENTER!",
        60,
        0.05,
    )?;
    input_pulse::run(config, |event| match event {
        RunEvent::CommandComplete => println!("Input dispatched."),
        other => println!("{other:?}"),
    })
}
