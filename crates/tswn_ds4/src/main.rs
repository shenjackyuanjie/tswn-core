//! DS4 的统一 CLI 出口，流程实现由 library 提供。

fn main() {
    if let Err(err) = tswn_ds4::run_cli(std::env::args().skip(1)) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
