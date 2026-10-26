# apps/desktop

Not scaffolded yet. When the daemon has a stable `/v1/state` stream,
create the app with:

```
cd apps/desktop
pnpm create tauri-app@latest . --template vanilla-ts
```

Then wire the window to `transparent: true`, `decorations: false`,
`alwaysOnTop: true` in `tauri.conf.json` and subscribe to the daemon.
Keep the renderer free of host-specific code; it only knows about
`sidepaw_core` states and pet packs.
