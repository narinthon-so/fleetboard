mod cli;

fn main() {
    std::process::exit(cli::run(std::env::args_os().collect()));
}
