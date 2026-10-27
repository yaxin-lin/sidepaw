//! The few strings the Rust side shows (tray menu, notifications).
//! Everything in the web views goes through `src/i18n.ts`; the two
//! tables must say the same things. The language is `[ui] language`
//! from config.toml, or the system's when that says `system`.

use sidepaw_core::State;
use sidepaw_daemon::config::UiLanguage;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    ZhCn,
}

// 0 follows the system; the setting is applied at startup and whenever
// the setup window changes it, so the tray refresher sees it next tick.
static SETTING: AtomicU8 = AtomicU8::new(0);

pub fn apply_setting(setting: UiLanguage) {
    SETTING.store(
        match setting {
            UiLanguage::System => 0,
            UiLanguage::En => 1,
            UiLanguage::ZhCn => 2,
        },
        Ordering::Relaxed,
    );
}

pub fn lang() -> Lang {
    match SETTING.load(Ordering::Relaxed) {
        1 => Lang::En,
        2 => Lang::ZhCn,
        _ => system(),
    }
}

fn system() -> Lang {
    static LANG: OnceLock<Lang> = OnceLock::new();
    *LANG.get_or_init(|| {
        let tag = sys_locale::get_locale().unwrap_or_default().to_lowercase();
        if tag.starts_with("zh") {
            Lang::ZhCn
        } else {
            Lang::En
        }
    })
}

fn pick(en: &'static str, zh: &'static str) -> &'static str {
    match lang() {
        Lang::En => en,
        Lang::ZhCn => zh,
    }
}

pub fn no_sessions() -> &'static str {
    pick("No active sessions", "没有活跃会话")
}
pub fn dnd() -> &'static str {
    pick("Do Not Disturb", "勿扰")
}
pub fn mini() -> &'static str {
    pick("Mini Mode (park at the edge)", "迷你模式（停在屏幕边）")
}
pub fn size() -> &'static str {
    pick("Size", "尺寸")
}
pub fn size_name(size: sidepaw_daemon::config::PetSize) -> &'static str {
    use sidepaw_daemon::config::PetSize;
    match size {
        PetSize::S => pick("Small", "小"),
        PetSize::M => pick("Medium", "中"),
        PetSize::L => pick("Large", "大"),
    }
}
pub fn autostart() -> &'static str {
    pick("Start at Login", "开机自启")
}
pub fn history() -> &'static str {
    pick("Usage History...", "用量历史…")
}
pub fn chat() -> &'static str {
    pick("Chat...", "聊天…")
}
pub fn setup() -> &'static str {
    pick("Setup...", "设置…")
}
pub fn hide_pet() -> &'static str {
    pick("Hide pet", "隐藏宠物")
}
pub fn show_pet() -> &'static str {
    pick("Show pet", "显示宠物")
}
pub fn quit() -> &'static str {
    pick("Quit sidepaw", "退出 sidepaw")
}

pub fn state(state: State) -> &'static str {
    match state {
        State::Waiting => pick("needs you", "等你"),
        State::Error => pick("error", "出错"),
        State::Tool(_) => pick("running a tool", "跑工具"),
        State::Thinking => pick("thinking", "思考中"),
        State::Done => pick("done", "完成"),
        State::Idle => pick("idle", "空闲"),
        State::Sleeping => pick("asleep", "睡着了"),
    }
}

/// Minutes only: the tray menu is rebuilt whenever this text changes.
pub fn duration(ms: u64) -> String {
    let m = ms / 60_000;
    match (lang(), m) {
        (Lang::En, 0) => "<1 min".into(),
        (Lang::En, m) if m < 60 => format!("{m} min"),
        (Lang::En, m) => format!("{}h {:02}m", m / 60, m % 60),
        (Lang::ZhCn, 0) => "不到 1 分钟".into(),
        (Lang::ZhCn, m) if m < 60 => format!("{m} 分钟"),
        (Lang::ZhCn, m) => format!("{} 小时 {} 分", m / 60, m % 60),
    }
}

/// `claude-code:abc…` reads as "Claude Code" to a person.
pub fn host(key: &str) -> &str {
    match key.split(':').next().unwrap_or("") {
        "claude-code" => "Claude Code",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        "dsh" => "DeepSeek Harness",
        "kimi" => "Kimi Code",
        "gemini" => "Gemini CLI",
        "cursor" => "Cursor",
        "" => pick("An agent", "某个 agent"),
        other => other,
    }
}

/// Notification body for a state that warrants one, `None` otherwise.
pub fn notification(key: &str, state: State) -> Option<String> {
    let what = match state {
        State::Waiting => pick("needs your approval", "等你批准"),
        State::Error => pick("hit an error", "出错了"),
        State::Done => pick("finished a turn", "完成了一轮"),
        _ => return None,
    };
    Some(match lang() {
        Lang::En => format!("{} {what}", host(key)),
        Lang::ZhCn => format!("{}{what}", host(key)),
    })
}

pub fn test_notification() -> &'static str {
    pick(
        "Notifications work. \"Claude Code needs your approval\" would look like this.",
        "通知正常。「Claude Code 等你批准」会长这样。",
    )
}
