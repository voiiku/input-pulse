use input_pulse::{InputPulseConfig, TargetMode};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let target = args.next().ok_or_else(|| {
        "usage: cargo run --example window -- <window-title-substring>".to_owned()
    })?;
    if args.next().is_some() {
        return Err("expected exactly one window-title substring".to_owned());
    }

    let config = InputPulseConfig::new(
        TargetMode::Window,
        target,
        "!T!Input Pulse demo!ESC!",
        60,
        0.05,
    )?;
    input_pulse::run(config, |_| {})
}
