fn main() {
    let exit_code = codedash::cli::run().unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        1
    });
    std::process::exit(exit_code);
}
