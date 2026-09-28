import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri 开发需固定端口、strictPort、不自动开浏览器
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: false,
  },
  build: {
    outDir: "dist",
    target: "es2021",
  },
});
