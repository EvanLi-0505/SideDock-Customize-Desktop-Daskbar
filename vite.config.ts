import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";
import { readdirSync, existsSync } from "node:fs";
import { resolve } from "node:path";

const uiRoot = fileURLToPath(new URL("./src/ui", import.meta.url));

/** Every folder under `src/ui/apps` with an index.html is an independent webview app. */
function discoverApps(): Record<string, string> {
  const appsDir = resolve(uiRoot, "apps");
  const inputs: Record<string, string> = {};
  for (const entry of readdirSync(appsDir, { withFileTypes: true })) {
    const html = resolve(appsDir, entry.name, "index.html");
    if (entry.isDirectory() && existsSync(html)) {
      inputs[entry.name] = html;
    }
  }
  return inputs;
}

export default defineConfig({
  root: uiRoot,
  base: "./",
  clearScreen: false,
  plugins: [svelte({ configFile: fileURLToPath(new URL("./svelte.config.js", import.meta.url)) })],
  resolve: {
    alias: {
      "@shared": resolve(uiRoot, "shared"),
    },
  },
  server: {
    port: 3580,
    strictPort: true,
  },
  build: {
    outDir: fileURLToPath(new URL("./dist", import.meta.url)),
    emptyOutDir: true,
    target: "es2022",
    chunkSizeWarningLimit: 1024,
    rollupOptions: {
      input: discoverApps(),
    },
  },
});
