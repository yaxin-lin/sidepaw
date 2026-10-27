//! The model API key. It is the one secret sidepaw holds, so it never
//! touches `config.toml`, SQLite or logs: it lives in the OS credential
//! store under one fixed entry, and `SIDEPAW_API_KEY` in the environment
//! overrides it for headless use and CI.

use anyhow::{Context, Result};

const SERVICE: &str = "sidepaw";
const ENTRY: &str = "model-api-key";
const ENV: &str = "SIDEPAW_API_KEY";

fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, ENTRY).context("credential store")
}

/// Where the key the app would use comes from, for the setup window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum KeySource {
    Env,
    Keyring,
    None,
}

pub fn source() -> KeySource {
    if std::env::var_os(ENV).is_some_and(|v| !v.is_empty()) {
        return KeySource::Env;
    }
    match entry().and_then(|e| Ok(e.get_password()?)) {
        Ok(_) => KeySource::Keyring,
        Err(_) => KeySource::None,
    }
}

pub fn get() -> Result<Option<String>> {
    if let Some(v) = std::env::var_os(ENV) {
        let v = v.to_string_lossy().trim().to_owned();
        if !v.is_empty() {
            return Ok(Some(v));
        }
    }
    match entry()?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e).context("read the API key from the credential store"),
    }
}

pub fn set(key: &str) -> Result<()> {
    let key = key.trim();
    if key.is_empty() {
        return clear();
    }
    entry()?
        .set_password(key)
        .context("store the API key in the credential store")
}

pub fn clear() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).context("remove the API key from the credential store"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Touches the real credential store, so it only runs on request:
    // `cargo test -p sidepaw-daemon -- --ignored keyring`.
    #[test]
    #[ignore]
    fn keyring_round_trip() {
        std::env::remove_var(ENV);
        clear().unwrap();
        assert_eq!(get().unwrap(), None);
        assert_eq!(source(), KeySource::None);
        set("sk-test-123").unwrap();
        assert_eq!(get().unwrap().as_deref(), Some("sk-test-123"));
        assert_eq!(source(), KeySource::Keyring);
        set("  ").unwrap();
        assert_eq!(get().unwrap(), None);
    }

    #[test]
    fn env_wins_and_blank_env_is_ignored() {
        // The env var is process-wide; keep this the only test that sets it.
        std::env::set_var(ENV, "  from-env ");
        assert_eq!(get().unwrap().as_deref(), Some("from-env"));
        assert_eq!(source(), KeySource::Env);
        std::env::remove_var(ENV);
    }
}
