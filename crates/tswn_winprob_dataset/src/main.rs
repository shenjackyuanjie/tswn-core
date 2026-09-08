use clap::Parser;

fn main() {
    if let Err(error) = tswn_winprob_dataset::run(tswn_winprob_dataset::Cli::parse()) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
