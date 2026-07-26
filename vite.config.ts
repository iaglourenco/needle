import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri expects a fixed port and to fail if it's taken, plus a specific host.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
  },
});
