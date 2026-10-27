//! `sidepaw` binary. One executable serves several roles so users never
//! install a second tool: the headless daemon (`serve`), the stdin-to-HTTP
//! shim hosts call from their hook config (`hook`), the hook snippet
//! printer (`install`) and the command wrapper (`run`). All of it lives
//! in the library's `cli` module, which the desktop app runs too.

fn main() {
    std::process::exit(sidepaw_daemon::cli::main())
}
