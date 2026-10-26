# apps/desktop

Tauri 2 app: a transparent, borderless, always-on-top window that draws
the pet and embeds the ingest server (ADR 6). Vanilla TypeScript, no
framework.

```
pnpm install
pnpm gen:types        # TypeScript types from the Rust model, into src/generated
pnpm tauri dev        # runs vite and the app
pnpm tauri build      # installer under src-tauri/target/release/bundle
```

The renderer only knows about `sidepaw_core` states and the pet pack
layout. It never reads host files; events reach it through the
embedded server. Packs are served from the repository's `pets/` folder
in development and copied into the bundle for release.
