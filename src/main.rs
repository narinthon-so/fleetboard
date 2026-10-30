fn main() {
    std::process::exit(fleetboard::cli::run(std::env::args_os().collect()));
}
