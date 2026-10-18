import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed dev port and must not have Vite clear its log output.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "chrome120",
    outDir: "dist",
    emptyOutDir: true,
    // One page per Tauri window type.
    rollupOptions: { input: { main: "index.html", popup: "popup.html" } },
  },
});
