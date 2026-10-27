// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// With a subcommand (`sidepaw run -- npm test`, `sidepaw hook ...`) the
// app is the command line tool; without one it is the pet.
fn main() {
    let cli = std::env::args().nth(1).is_some_and(|a| {
        sidepaw_daemon::cli::SUBCOMMANDS.contains(&a.as_str()) || a.starts_with('-')
    });
    if cli {
        #[cfg(windows)]
        attach_console();
        std::process::exit(sidepaw_daemon::cli::main());
    }
    sidepaw_desktop_lib::run()
}

// A release build is a GUI program on Windows and starts without a
// console; borrowing the calling terminal's makes output and Ctrl+C
// work for the CLI.
#[cfg(windows)]
fn attach_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}
