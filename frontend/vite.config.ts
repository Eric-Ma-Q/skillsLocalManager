import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5732,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
});
