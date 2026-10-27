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
    /// The credential store could not be read (locked, no backend);
    /// `get` will fail the same way, so the UI should say so.
    Unavailable,
}

fn from_env() -> Option<String> {
    env_key(std::env::var(ENV).ok())
}

// A blank variable counts as unset, so `SIDEPAW_API_KEY= sidepaw` does
// not shadow a stored key with nothing.
fn env_key(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

pub fn source() -> KeySource {
    if from_env().is_some() {
        return KeySource::Env;
    }
    match entry().and_then(|e| Ok(e.get_password()?)) {
        Ok(_) => KeySource::Keyring,
        Err(e) if matches!(e.downcast_ref(), Some(keyring::Error::NoEntry)) => KeySource::None,
        Err(e) => {
            tracing::warn!("credential store: {e:#}");
            KeySource::Unavailable
        }
    }
}

pub fn get() -> Result<Option<String>> {
    if let Some(v) = from_env() {
        return Ok(Some(v));
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
        assert!(from_env().is_none(), "unset SIDEPAW_API_KEY for this test");
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
    fn env_value_is_trimmed_and_blank_means_unset() {
        assert_eq!(
            env_key(Some("  from-env ".into())).as_deref(),
            Some("from-env")
        );
        assert_eq!(env_key(Some("   ".into())), None);
        assert_eq!(env_key(None), None);
    }
}
