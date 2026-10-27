import { defineConfig } from "vite";

// Tauri expects a fixed port and serves the built `dist` in production.
// `publicDir` points at the repository's bundled pet packs so
// `/placeholder/pet.json` resolves both in dev and in the bundle.
export default defineConfig({
  clearScreen: false,
  publicDir: "../../pets",
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    rollupOptions: {
      input: { pet: "index.html", setup: "setup.html", chat: "chat.html", history: "history.html" },
    },
    target: ["es2022", "chrome105", "safari13"],
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
