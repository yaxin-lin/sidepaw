# sidepaw

[English](README.md)

一只坐在桌面上、看着你的 AI 编程 agent 干活的桌宠。

sidepaw 会对 Claude Code、Codex、OpenCode、DeepSeek Harness 和 Kimi
Code 正在做的事做出反应：思考中、运行工具、等你批准、完成、出错。你可以
给它任何形象（它读取 Codex 的宠物包格式）和任何性格，而且它只看你允许它
看的那部分会话内容。

状态：pre-alpha，尚未发布任何版本。设计见
[docs/architecture.md](docs/architecture.md)，开发顺序见
[docs/roadmap.md](docs/roadmap.md)，决策记录见 [docs/adr](docs/adr)。

## 为什么再做一个

已经有不少针对编程 agent 的桌宠（petdex、agentpet、CoPet、dsh-pet）。
sidepaw 的不同点有三个：

- 你自己的角色。一张参考图加一句话就能生成一个带形象和性格的宠物包，
  对所有支持的宿主都有效，不只是 Codex。
- 有上下文的聊天。宠物知道你的 agent 今天做了什么，所以「刚才那次为什么
  失败」能得到真实的回答。
- 权限分级。从「只看状态」到「可以给 agent 发消息」共五级，可按宿主和
  按项目设置，自带脱敏和审计日志。

## 支持的宿主

| 宿主 | 接入方式 | 状态 |
| --- | --- | --- |
| Claude Code | `http` hook | 适配器已完成；`sidepaw install --host claude-code` 打印 hook 配置 |
| Codex | 通过 `sidepaw hook` 转发 | 计划中，0.2 |
| DeepSeek Harness | 读取会话日志 | 计划中 |
| OpenCode | `/event` SSE | 计划中 |
| Kimi Code | 通过 `sidepaw hook` 转发 | 计划中 |

## 目录

```
crates/sidepaw-core     事件模型、状态机、宠物包格式（无 I/O）
crates/sidepaw-daemon   库：本地接收服务、SQLite 存储、宿主适配器；
                        另有一个很薄的 `sidepaw` 二进制
apps/desktop            Tauri 2 应用：桌宠窗口和内嵌服务
docs/                   架构、路线图、ADR、接口契约
tools/                  纯标准库脚本：占位精灵图、应用图标
pets/                   内置宠物包（目前只有生成的占位包）
```

## 构建

需要 Rust 1.80 以上、Node 22 和 pnpm。Linux 上 Tauri 需要的系统库见
`.github/workflows/ci.yml`。

```
pnpm install
pnpm gen:types                          # 从 Rust 模型生成 TypeScript 类型
cargo test --workspace
pnpm --filter sidepaw-desktop tauri dev # 启动桌宠窗口
```

宠物的台词来自素材包里的 `lines.json`，不需要模型。想要自己的人格，设置
窗口里有人格编辑器（滑块、标签、自由文本）；配好模型服务商后（DeepSeek、
OpenAI、Anthropic、Ollama 或任意 OpenAI 兼容地址，key 存系统钥匙串），
点「生成台词」就会按这个人格写一份 `lines.json` 到 `~/.sidepaw/pets/<id>/`；
双击宠物打开聊天窗口，可以问它 agent 今天都干了什么。每次远端调用都会先
显示将发送的内容，并列在「外发记录」里。

窗口会显示 `pets/` 里的占位宠物。首次运行或在小球上右键会打开设置窗口，
一键把 Claude Code 的 hook 写进 `~/.claude/settings.json`（旁边会留一份
备份）。终端里等价的命令是 `sidepaw install --host claude-code --apply`。

## 常见问题

- Claude Code 提示 `PreToolUse:Bash hook error` / `socket hang up`：
  发往 `127.0.0.1` 的请求走了系统代理（Clash 全局模式等）。设置窗口会在
  `settings.json` 的 `env` 里加上 `NO_PROXY=127.0.0.1,localhost`；如果你
  在别处设了代理，也要把回环地址排除。改完要开新的 Claude Code 会话，
  hook 配置只在启动时读取。
- Claude Code 报 `connect ECONNREFUSED 127.0.0.1:<端口>`：hook 指向了应用
  已不再监听的端口。sidepaw 启动时会自动重写自己的 hook，重启一下（或在
  设置窗口点更新），然后新开一个 Claude Code 会话。

## 参与贡献

见 [CONTRIBUTING.md](CONTRIBUTING.md)。Issue 和 PR 用英文或中文都可以。

## 许可证

MIT。`pets/` 下的宠物素材各有自己的许可证，见每个包的 `pet.json`。
