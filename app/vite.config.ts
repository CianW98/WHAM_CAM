import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Settings recommended by Tauri: fixed port, and don't watch the Rust folder.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // Desktop app loaded from disk: one larger bundle (three.js) is fine.
  build: { chunkSizeWarningLimit: 1500 },
  server: {
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
