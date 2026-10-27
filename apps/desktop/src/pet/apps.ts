/** What kind of app is in front, from its bundle id (macOS) or
 *  executable name (Windows). Only names are known, never content, so
 *  this is a guess the pet can act on lightly: hush in a meeting, move
 *  to music, comment now and then on what you switched to. */
export type AppKind = "meeting" | "music" | "video" | "terminal" | "editor" | "browser" | "chat";

// Lowercase, matched against the bundle id or exe name; a trailing "*"
// matches a prefix (JetBrains ships one id per product).
const KINDS: Record<AppKind, string[]> = {
  meeting: ["us.zoom.xos", "zoom.exe", "com.tencent.meeting", "wemeetapp.exe", "com.microsoft.teams", "com.microsoft.teams2", "ms-teams.exe", "teams.exe", "com.apple.facetime", "com.webex.meetingmanager", "cisco.webex.*", "webex.exe", "com.ringcentral.glip"],
  music: ["com.spotify.client", "spotify.exe", "com.apple.music", "com.netease.163music", "cloudmusic.exe", "com.tencent.qqmusicmac", "qqmusic.exe", "com.kugou.mac.kugoumusic", "kugou.exe"],
  video: ["com.apple.tv", "org.videolan.vlc", "vlc.exe", "com.colliderli.iina", "potplayermini64.exe", "com.bilibili.*", "com.netflix.*", "mpv.exe", "io.mpv"],
  terminal: ["com.apple.terminal", "com.googlecode.iterm2", "dev.warp.warp-stable", "com.mitchellh.ghostty", "net.kovidgoyal.kitty", "io.alacritty", "org.alacritty", "co.zeit.hyper", "windowsterminal.exe", "wt.exe", "alacritty.exe", "wezterm-gui.exe", "com.github.wez.wezterm"],
  editor: ["com.microsoft.vscode", "code.exe", "com.todesktop.230313mzl4w4u92", "cursor.exe", "dev.zed.zed", "com.jetbrains.*", "idea64.exe", "pycharm64.exe", "webstorm64.exe", "com.sublimetext.4", "sublime_text.exe", "com.apple.dt.xcode"],
  browser: ["com.google.chrome", "chrome.exe", "com.apple.safari", "org.mozilla.firefox", "firefox.exe", "company.thebrowser.browser", "arc.exe", "com.microsoft.edgemac", "msedge.exe", "com.brave.browser", "brave.exe"],
  chat: ["com.tencent.xinwechat", "wechat.exe", "weixin.exe", "com.tinyspeck.slackmacgap", "slack.exe", "com.hnc.discord", "discord.exe", "ru.keepcoder.telegram", "telegram.exe", "com.alibaba.dingtalkmac", "dingtalk.exe", "com.bytedance.macos.feishu", "feishu.exe", "lark.exe", "com.tencent.qq", "qq.exe"],
};

export function categorize(app: { name: string; id: string } | null): AppKind | null {
  if (!app) return null;
  const id = app.id.toLowerCase();
  for (const [kind, ids] of Object.entries(KINDS) as [AppKind, string[]][]) {
    if (ids.some((p) => (p.endsWith("*") ? id.startsWith(p.slice(0, -1)) : id === p))) return kind;
  }
  return null;
}
