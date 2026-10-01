import { defineConfig } from "vite";

// Fixed port so `tauri.conf.json` -> `build.devUrl` can point at it.
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
});
