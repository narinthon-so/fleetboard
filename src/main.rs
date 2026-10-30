mod cli;
mod config;

fn main() {
    std::process::exit(cli::run(std::env::args_os().collect()));
}
