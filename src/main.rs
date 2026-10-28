mod cli;

fn main() {
    if let Err(error) = cli::run() {
        eprintln!("Application error: {error}");
        std::process::exit(1);
    }
}
