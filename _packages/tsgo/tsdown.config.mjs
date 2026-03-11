import { defineConfig } from "tsdown"

export default defineConfig({
  entry: {
    "effect-tsgo": "./src/cli.ts",
  },
  inlineOnly: false,
  outDir: "./bin",
  format: ["cjs"],
  platform: "node",
  target: "node22",
  dts: false,
  clean: true,
  outExtensions: () => ({
    js: ".js",
  }),
  banner: {
    js: "#!/usr/bin/env node",
  },
})
