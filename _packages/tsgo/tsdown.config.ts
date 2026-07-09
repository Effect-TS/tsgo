import * as NodeFileSystem from "@effect/platform-node/NodeFileSystem"
import * as NodePath from "@effect/platform-node/NodePath"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Layer from "effect/Layer"
import * as Path from "effect/Path"
import { defineConfig } from "tsdown"

export default defineConfig({
  entry: {
    "effect-tsgo": "./src/cli.ts",
  },
  inlineOnly: false,
  outDir: "./dist",
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
  onSuccess() {
    const program = Effect.gen(function*() {
      const fs = yield* FileSystem.FileSystem
      const path = yield* Path.Path

      const readme = yield* fs.readFileString("../../README.md")
      yield* fs.writeFileString(path.join("README.md"), readme)
    }).pipe(Effect.provide(Layer.merge(NodeFileSystem.layer, NodePath.layerPosix)))

    return Effect.runPromise(program)
  },
})
