//! Secret redaction. Runs on every free-text field before storage and
//! cannot be disabled. Patterns err on the side of catching too much;
//! a redacted file path is annoying, a stored API key is a leak.

use regex::Regex;
use sidepaw_core::Event;
use std::sync::LazyLock;

struct Pattern {
    label: &'static str,
    re: Regex,
}

static PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    let p = |label, re| Pattern {
        label,
        re: Regex::new(re).expect("static regex"),
    };
    vec![
        // Vendor key prefixes: OpenAI, Anthropic, GitHub, Slack, AWS, Google.
        p(
            "key",
            r"\b(?:sk|rk|pk)-[A-Za-z0-9_-]{16,}|\bsk-ant-[A-Za-z0-9_-]{16,}|\bgh[pousr]_[A-Za-z0-9]{20,}|\bxox[baprs]-[A-Za-z0-9-]{10,}|\bAKIA[0-9A-Z]{16}\b|\bAIza[0-9A-Za-z_-]{35}\b",
        ),
        p(
            "token",
            r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}|\b(?:token|authorization)\s*[:=]\s*[A-Za-z0-9._~+/=-]{16,}",
        ),
        p(
            "password",
            r"(?i)\b(?:password|passwd|pwd)\s*[:=]\s*\S+|://[^/\s:]+:[^@\s]+@",
        ),
        p(
            "email",
            r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
        ),
        p(
            "ip",
            r"\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3})\b",
        ),
    ]
});

pub fn text(input: &str) -> String {
    let mut out = input.to_string();
    for p in PATTERNS.iter() {
        if p.re.is_match(&out) {
            out =
                p.re.replace_all(&out, format!("[REDACTED:{}]", p.label))
                    .into_owned();
        }
    }
    out
}

pub fn event(event: &mut Event) {
    if let Some(t) = event.text.as_mut() {
        *t = text(t);
    }
    if let Some(e) = event.error.as_mut() {
        if let Some(m) = e.message.as_mut() {
            *m = text(m);
        }
    }
    if let Some(raw) = event.raw.as_mut() {
        value(raw);
    }
}

fn value(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::String(s) => *s = text(s),
        serde_json::Value::Array(items) => items.iter_mut().for_each(value),
        serde_json::Value::Object(map) => map.values_mut().for_each(value),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_vendor_keys() {
        let s = "export OPENAI_API_KEY=sk-abcdefghijklmnopqrstuvwxyz123456";
        assert_eq!(text(s), "export OPENAI_API_KEY=[REDACTED:key]");
        let s = "token ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        assert!(text(s).contains("[REDACTED:key]"));
    }

    #[test]
    fn catches_bearer_and_passwords() {
        assert!(text("Authorization: Bearer abcdefghijklmnopqrstu").contains("[REDACTED:token]"));
        assert!(text("password=hunter2hunter2").contains("[REDACTED:password]"));
        assert_eq!(
            text("postgres://alice:s3cret@db.internal/app"),
            "postgres[REDACTED:password]db.internal/app"
        );
    }

    #[test]
    fn catches_email_and_private_ip() {
        assert_eq!(
            text("mail me at a.b@example.com"),
            "mail me at [REDACTED:email]"
        );
        assert_eq!(text("ssh 192.168.1.20"), "ssh [REDACTED:ip]");
        assert_eq!(text("ssh 8.8.8.8"), "ssh 8.8.8.8");
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        for s in [
            "npm install",
            "cargo test --workspace",
            "git commit -m 'fix: sk-ip bad path'",
            "/home/me/project/src/main.rs",
        ] {
            assert_eq!(text(s), s);
        }
    }

    #[test]
    fn walks_raw_json() {
        let mut v = serde_json::json!({
            "cmd": "curl -H 'Authorization: Bearer abcdefghijklmnopqrstu'",
            "nested": [{"k": "sk-abcdefghijklmnopqrstuvwxyz123456"}],
            "n": 1
        });
        value(&mut v);
        let s = v.to_string();
        assert!(s.contains("[REDACTED:token]") && s.contains("[REDACTED:key]"));
        assert!(!s.contains("sk-abc"));
    }
}
