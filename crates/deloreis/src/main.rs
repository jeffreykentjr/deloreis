//! Thin executable wrapper around the `deloreis` library's CLI entry point.

fn main() {
    std::process::exit(deloreis::run_cli());
}
