use clap::Parser;

fn main() {
    if let Err(error) = tswn_pwp::run(tswn_pwp::Cli::parse()) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
