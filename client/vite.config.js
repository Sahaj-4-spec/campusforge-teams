import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const API_TARGET = "http://localhost:5000";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          const normalized = id.replace(/\\/g, "/");
          if (
            normalized.includes("/node_modules/react/") ||
            normalized.includes("/node_modules/react-dom/") ||
            normalized.includes("/node_modules/scheduler/")
          ) return "react-vendor";
          if (normalized.includes("/node_modules/@dnd-kit/")) return "dnd-kit";
          return undefined;
        }
      }
    }
  },
  server: {
    host: "0.0.0.0",
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        target: API_TARGET,
        changeOrigin: true,
      },
      "/ws": {
        target: API_TARGET,
        changeOrigin: true,
        ws: true,
      },
    },
  },
});
