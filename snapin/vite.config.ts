import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 期望固定端口，dev 时前端跑在 1420
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  // 避免 Vite 屏蔽 Tauri 的 Rust 错误
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // 忽略 src-tauri 目录，避免 Rust 改动触发前端重载
      ignored: ["**/src-tauri/**"],
    },
  },
});
