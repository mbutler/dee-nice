fn main() {
    if let Err(error) = dee_nice::run() {
        eprintln!("dee-nice: {error}");
        std::process::exit(1);
    }
}
