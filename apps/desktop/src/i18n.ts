/** UI strings. The language follows the system (navigator.language);
 *  pet lines follow the pack instead, see pack.ts. */
export type Lang = "en" | "zh-CN";

const STRINGS = {
  en: {
    "setup.title": "sidepaw setup",
    "setup.sub": "Connect your coding agents. Nothing is sent anywhere but this machine.",
    "server": "Server",
    "server.address": "Address",
    "server.data": "Data",
    "today": "Today",
    "today.sessions": "Sessions",
    "today.turns": "Turns",
    "today.tools": "Tool calls",
    "today.errors": "Errors",
    "today.waiting": "Waited on you",
    "today.top": "Most used",
    "level": "Permission level",
    "level.sub": "What the pet may see and do. Lower is safer; the default is 1. Per-host and per-project overrides live in ~/.sidepaw/config.toml.",
    "level.0": "Observe",
    "level.0.text": "Only the state: thinking, tool, waiting, done. Animation and local lines.",
    "level.1": "Record",
    "level.1.text": "Plus tool names, error types, token counts. Daily report and counters.",
    "level.2": "Read",
    "level.2.text": "Plus file paths, command text, error messages (secrets redacted). Needed to answer \"why did that fail\".",
    "level.3": "Notify",
    "level.3.text": "Same as Read, plus system notifications when an agent waits for you, errors or finishes.",
    "level.4": "Act",
    "level.4.text": "Same as Read, plus answering agents (approve, deny, send a message), each confirmed.",
    "level.saved": "Level {n} saved to config.toml.",
    "notify.test": "Test notification",
    "notify.sent": "Sent. Nothing shown? Notifications need level 3 for real events, and on macOS a bundled build.",
    "cc.installed": "Hooks are installed and point at this server. Start a Claude Code session and the pet reacts.",
    "cc.stale": "Hooks are installed but point at an old port or token. Install again to update them.",
    "cc.absent": "Claude Code has a settings file but no sidepaw hooks yet.",
    "cc.no_file": "No Claude Code settings file yet; installing will create it.",
    "cc.preview": "What will be written",
    "cc.proxy": "env.NO_PROXY gets 127.0.0.1,localhost appended so a system proxy (Clash, Surge, ...) cannot swallow the loopback request. Other values in it are kept.",
    "cc.install": "Install hooks",
    "cc.update": "Update hooks",
    "cc.remove": "Remove",
    "cc.written": "Written. A backup is next to the file.",
    "cc.removed": "Removed sidepaw's hooks; everything else untouched.",
    "others": "Other hosts",
    "others.text": "Codex, OpenCode, DeepSeek Harness and Kimi Code are on the roadmap. See docs/roadmap.md.",
    "status.installed": "installed",
    "status.stale": "stale",
    "status.absent": "not installed",
    "status.no_file": "no file",
    "state.sleeping": "asleep",
    "state.idle": "idle",
    "state.done": "done",
    "state.thinking": "thinking",
    "state.tool": "tool",
    "state.error": "error",
    "state.waiting": "waiting",
    "starting": "starting",
  },
  "zh-CN": {
    "setup.title": "sidepaw 设置",
    "setup.sub": "接入你的编程 agent。所有数据都留在这台机器上。",
    "server": "服务",
    "server.address": "地址",
    "server.data": "数据目录",
    "today": "今天",
    "today.sessions": "会话",
    "today.turns": "轮次",
    "today.tools": "工具调用",
    "today.errors": "出错",
    "today.waiting": "等你批准",
    "today.top": "最常用",
    "level": "权限档位",
    "level.sub": "宠物能看到什么、能做什么。数字越小越保守，默认 1。按宿主和按项目的覆盖写在 ~/.sidepaw/config.toml。",
    "level.0": "旁观",
    "level.0.text": "只有状态：思考、工具、等待、完成。动画和本地台词。",
    "level.1": "记录",
    "level.1.text": "加上工具名、错误类型、token 数。日报和计数。",
    "level.2": "阅读",
    "level.2.text": "加上文件路径、命令文本、错误信息（敏感信息已脱敏）。回答「为什么挂了」需要这一档。",
    "level.3": "通知",
    "level.3.text": "同阅读，加上系统通知：agent 等你批准、出错、完成时提醒。",
    "level.4": "操作",
    "level.4.text": "同阅读，加上替你回答 agent（批准、拒绝、发消息），每次确认。",
    "level.saved": "档位 {n} 已写入 config.toml。",
    "notify.test": "测试通知",
    "notify.sent": "已发送。没看到？真实事件需要档位 3，macOS 上需要打包后的应用。",
    "cc.installed": "Hook 已安装并指向当前服务。开一个 Claude Code 会话，宠物就会动。",
    "cc.stale": "Hook 已安装但指向旧的端口或 token，再装一次即可更新。",
    "cc.absent": "Claude Code 有设置文件，但还没有 sidepaw 的 hook。",
    "cc.no_file": "还没有 Claude Code 设置文件，安装时会创建。",
    "cc.preview": "将写入的内容",
    "cc.proxy": "会把 127.0.0.1,localhost 追加到 env.NO_PROXY，防止系统代理（Clash、Surge 等）吞掉本机回环请求。原有的值保留。",
    "cc.install": "安装 hook",
    "cc.update": "更新 hook",
    "cc.remove": "移除",
    "cc.written": "已写入，备份在同目录。",
    "cc.removed": "已移除 sidepaw 的 hook，其他内容未动。",
    "others": "其他宿主",
    "others.text": "Codex、OpenCode、DeepSeek Harness、Kimi Code 在路线图上，见 docs/roadmap.md。",
    "status.installed": "已安装",
    "status.stale": "已过期",
    "status.absent": "未安装",
    "status.no_file": "无文件",
    "state.sleeping": "睡着了",
    "state.idle": "空闲",
    "state.done": "完成",
    "state.thinking": "思考中",
    "state.tool": "工具",
    "state.error": "出错",
    "state.waiting": "等你",
    "starting": "启动中",
  },
} as const;

export type Key = keyof (typeof STRINGS)["en"];

export function detectLang(tag = navigator.language): Lang {
  return tag.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

export function t(key: Key, vars: Record<string, string | number> = {}, lang: Lang = detectLang()): string {
  const table: Record<string, string> = STRINGS[lang];
  let s = table[key] ?? STRINGS.en[key];
  for (const [k, v] of Object.entries(vars)) s = s.replace(`{${k}}`, String(v));
  return s;
}

/** Fill every element carrying `data-i18n="<key>"`. */
export function applyToDom(root: ParentNode = document) {
  for (const el of root.querySelectorAll<HTMLElement>("[data-i18n]")) {
    const key = el.dataset.i18n as Key;
    el.textContent = t(key);
  }
  document.documentElement.lang = detectLang();
}
