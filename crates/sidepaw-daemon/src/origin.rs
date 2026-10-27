//! Where a session lives: the process that sent its hook requests and
//! that process's terminal. Hosts call us over loopback, so the peer
//! port of the request identifies their process, and the process
//! table says which tty it runs on. Resolved once per session, with
//! the system's own tools, so there is nothing to install.

use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Origin {
    pub pid: u32,
    /// Like `ttys003`, without `/dev/`; absent for a process without a
    /// controlling terminal (an IDE's own agent, a service).
    pub tty: Option<String>,
}

/// The process behind a loopback connection whose client port is
/// `peer_port`, or `None` when the tools are missing or it is gone.
pub fn resolve(peer_port: u16) -> Option<Origin> {
    let pid = peer_pid(peer_port)?;
    Some(Origin {
        pid,
        tty: tty_of(pid),
    })
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(target_os = "macos")]
fn peer_pid(port: u16) -> Option<u32> {
    let port_arg = format!("-iTCP:{port}");
    parse_lsof(
        &run("lsof", &["-nP", &port_arg, "-sTCP:ESTABLISHED"])?,
        port,
    )
}

#[cfg(target_os = "linux")]
fn peer_pid(port: u16) -> Option<u32> {
    parse_ss(&run("ss", &["-tnpH"])?, port)
}

#[cfg(target_os = "windows")]
fn peer_pid(port: u16) -> Option<u32> {
    parse_netstat(&run("netstat", &["-ano", "-p", "TCP"])?, port)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn peer_pid(_port: u16) -> Option<u32> {
    None
}

#[cfg(unix)]
fn tty_of(pid: u32) -> Option<String> {
    let out = run("ps", &["-o", "tty=", "-p", &pid.to_string()])?;
    let tty = out.trim();
    (!tty.is_empty() && tty != "??" && tty != "?")
        .then(|| tty.trim_start_matches("/dev/").to_owned())
}

#[cfg(not(unix))]
fn tty_of(_pid: u32) -> Option<String> {
    None
}

// `lsof` lists both ends of a loopback connection; the client is the
// row whose local address (before `->`) carries the client port.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_lsof(out: &str, port: u16) -> Option<u32> {
    let suffix = format!(":{port}->");
    out.lines().find_map(|line| {
        let mut cols = line.split_whitespace();
        let _command = cols.next()?;
        let pid = cols.next()?.parse().ok()?;
        let name = cols.last()?;
        let local = line.split_whitespace().find(|c| c.contains("->"))?;
        (local.contains(&suffix) || name.contains(&suffix)).then_some(pid)
    })
}

// `ss -tnpH`: `ESTAB 0 0 127.0.0.1:54017 127.0.0.1:59785 users:(("node",pid=1234,fd=20))`
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_ss(out: &str, port: u16) -> Option<u32> {
    let suffix = format!(":{port}");
    out.lines().find_map(|line| {
        let mut cols = line.split_whitespace();
        let _state = cols.next()?;
        let _recv = cols.next()?;
        let _send = cols.next()?;
        let local = cols.next()?;
        if !local.ends_with(&suffix) {
            return None;
        }
        let rest = line.split("pid=").nth(1)?;
        rest.split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()
    })
}

// `netstat -ano`: `  TCP    127.0.0.1:54017    127.0.0.1:59785    ESTABLISHED    1234`
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_netstat(out: &str, port: u16) -> Option<u32> {
    let suffix = format!(":{port}");
    out.lines().find_map(|line| {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 5 || cols[0] != "TCP" || !cols[1].ends_with(&suffix) {
            return None;
        }
        cols[4].parse().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsof_picks_the_client_side() {
        let out = "COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME\n\
                   sidepaw-d 100 me 16u IPv4 0x1 0t0 TCP 127.0.0.1:59785->127.0.0.1:54017 (ESTABLISHED)\n\
                   node 4242 me 20u IPv4 0x2 0t0 TCP 127.0.0.1:54017->127.0.0.1:59785 (ESTABLISHED)\n";
        assert_eq!(parse_lsof(out, 54017), Some(4242));
        assert_eq!(parse_lsof(out, 1), None);
    }

    #[test]
    fn ss_and_netstat_find_the_pid_by_local_port() {
        let ss = "ESTAB 0 0 127.0.0.1:59785 127.0.0.1:54017 users:((\"sidepaw\",pid=100,fd=16))\n\
                  ESTAB 0 0 127.0.0.1:54017 127.0.0.1:59785 users:((\"node\",pid=4242,fd=20))\n";
        assert_eq!(parse_ss(ss, 54017), Some(4242));
        let netstat =
            "\nActive Connections\n\n  Proto  Local Address  Foreign Address  State  PID\n\
                       TCP    127.0.0.1:59785    127.0.0.1:54017    ESTABLISHED    100\n\
                       TCP    127.0.0.1:54017    127.0.0.1:59785    ESTABLISHED    4242\n";
        assert_eq!(parse_netstat(netstat, 54017), Some(4242));
        assert_eq!(parse_netstat(netstat, 9), None);
    }
}
