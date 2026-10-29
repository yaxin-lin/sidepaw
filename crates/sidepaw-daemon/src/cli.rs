//! The command line, shared by the `sidepaw` binary and the desktop
//! executable (which runs it when started with a subcommand), so an
//! installed app is also the CLI and the hook shim.

use crate::{home, hook, install, packs, run, server};
use anyhow::Result;
use clap::{Parser, Subcommand};
use sidepaw_core::Host;

#[derive(Parser)]
#[command(
    name = "sidepaw",
    version,
    about = "desktop pet daemon for AI coding agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon in the foreground.
    Serve {
        /// Port to listen on; 0 picks a free one and records it in state.json.
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
    /// Read one native hook payload from stdin and post it to the daemon.
    Hook {
        #[arg(long)]
        host: Host,
    },
    /// Print the hook configuration snippet for a host, or write it.
    Install {
        #[arg(long)]
        host: Host,
        /// Merge the snippet into the host's settings file (backs it up first).
        #[arg(long, conflicts_with = "remove")]
        apply: bool,
        /// Remove sidepaw's handlers from the host's settings file.
        #[arg(long)]
        remove: bool,
    },
    /// Install a pet pack from a zip or a folder (Codex or petdex format).
    Pack { path: std::path::PathBuf },
    /// Run a command and let the pet show it: busy while it runs, done
    /// or failed when it exits. Exits with the command's own status.
    Run {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
}

/// The subcommands, for callers that decide whether to run the CLI.
pub const SUBCOMMANDS: [&str; 5] = ["serve", "hook", "install", "run", "pack"];

/// Parse the process arguments, run, and return the exit code.
pub fn main() -> i32 {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    match dispatch(Cli::parse().command) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("sidepaw: {e:#}");
            1
        }
    }
}

fn dispatch(command: Command) -> Result<i32> {
    match command {
        Command::Serve { port } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(server::run(port))?;
            Ok(0)
        }
        Command::Hook { host } => hook::forward_stdin(host).map(|_| 0),
        Command::Run { command } => run::run(&command),
        Command::Pack { path } => {
            let home = home::dir()?;
            let info = if path.is_dir() {
                let source = packs::Source {
                    kind: "folder".into(),
                    ..Default::default()
                };
                packs::install_folder(&home, &path, source)?
            } else {
                let source = packs::Source {
                    kind: "zip".into(),
                    ..Default::default()
                };
                packs::install_zip(&home, &std::fs::read(&path)?, source)?
            };
            println!(
                "installed {} ({}); choose it in Setup > Look",
                info.display_name, info.id
            );
            Ok(0)
        }
        Command::Install {
            host,
            apply,
            remove,
        } => {
            if apply || remove {
                let report = if apply {
                    install::apply(host)?
                } else {
                    install::remove(host)?
                };
                println!("{}: {:?} ({})", host.as_str(), report.status, report.path);
            } else {
                println!("{}", hook::install_snippet(host)?);
            }
            Ok(0)
        }
    }
}
