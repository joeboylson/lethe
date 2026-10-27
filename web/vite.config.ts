import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// `npm run dev` talks to a running `lethe serve` for the API.
export default defineConfig({
  plugins: [react()],
  server: { proxy: { "/api": "http://127.0.0.1:7340" } },
  build: { chunkSizeWarningLimit: 1000 },
});
