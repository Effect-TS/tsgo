import * as NodeFileSystem from "@effect/platform-node/NodeFileSystem"
import * as NodePath from "@effect/platform-node/NodePath"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Layer from "effect/Layer"
import * as Path from "effect/Path"
import { defineConfig } from "tsdown"

const copyPackageFiles = () => {
  const program = Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const path = yield* Path.Path

    const readme = yield* fs.readFileString("../../README.md")
    yield* fs.writeFileString(path.join("README.md"), readme)

    const schemaJson = yield* fs.readFileString("../../schema.json")
    yield* fs.writeFileString(path.join("schema.json"), schemaJson)
  }).pipe(Effect.provide(Layer.merge(NodeFileSystem.layer, NodePath.layerPosix)))

  return Effect.runPromise(program)
}

export default defineConfig([
  {
    entry: {
      "effect-tsgo": "./src/cli/index.ts",
    },
    inlineOnly: false,
    outDir: "./dist",
    format: ["cjs"],
    platform: "node",
    target: "node22",
    dts: false,
    clean: true,
    outExtensions: () => ({
      js: ".cjs",
    }),
    banner: {
      js: "#!/usr/bin/env node",
    },
    onSuccess: copyPackageFiles,
  },
  {
    entry: {
      "experimental/oxlint/index": "./src/experimental/oxlint/plugin.ts",
    },
    inlineOnly: false,
    outDir: "./dist",
    format: ["esm"],
    platform: "node",
    target: "node22",
    tsconfig: "./tsconfig.src.json",
    dts: true,
    clean: false,
    external: ["#getExePath"],
    outExtensions: () => ({
      js: ".js",
      dts: ".d.ts",
    }),
  },
])
