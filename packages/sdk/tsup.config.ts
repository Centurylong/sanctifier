import { defineConfig } from "tsup";

export default defineConfig([
  {
    entry: { index: "src/node.ts" },
    outDir: "dist/node",
    format: ["esm", "cjs"],
    platform: "node",
    target: "node18",
    dts: false,
    sourcemap: true,
    clean: false,
    splitting: false,
    shims: true,
  },
  {
    entry: { index: "src/browser.ts" },
    outDir: "dist/browser",
    format: ["esm"],
    platform: "browser",
    target: "es2022",
    dts: false,
    sourcemap: true,
    clean: false,
    splitting: false,
  },
  {
    entry: { index: "src/index.ts" },
    outDir: "dist/types",
    format: ["esm"],
    dts: { only: true },
    clean: false,
  },
]);
