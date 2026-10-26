//! `sidepaw` binary. One executable serves three roles so users never
//! install a second tool: the headless daemon (`serve`), the stdin-to-HTTP
//! shim that hosts call from their hook config (`hook`), and the printer
//! for the hook snippets the setup wizard writes (`install`).
//!
//! All logic lives in the `sidepaw_daemon` library so the desktop app
//! can run the same server inside its own process.

use anyhow::Result;
use clap::{Parser, Subcommand};
use sidepaw_core::Host;
use sidepaw_daemon::{hook, server};

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
        #[arg(long, value_parser = parse_host)]
        host: Host,
    },
    /// Print the hook configuration snippet for a host.
    Install {
        #[arg(long, value_parser = parse_host)]
        host: Host,
    },
}

fn parse_host(s: &str) -> Result<Host, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| format!("unknown host {s:?}"))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    match Cli::parse().command {
        Command::Serve { port } => server::run(port).await,
        Command::Hook { host } => hook::forward_stdin(host),
        Command::Install { host } => {
            println!("{}", hook::install_snippet(host)?);
            Ok(())
        }
    }
}
