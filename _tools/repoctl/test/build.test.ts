import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import test from "node:test"
import {
  buildArtifact,
  buildTargets,
  componentArtifact,
  oxlintArtifacts,
  oxlintBuildTargets
} from "../src/build.ts"

test("maps release targets to Go platforms", () => {
  assert.deepEqual(buildTargets["darwin-x64"], { goos: "darwin", goarch: "amd64" })
  assert.deepEqual(buildTargets["win32-arm64"], { goos: "windows", goarch: "arm64" })
  assert.deepEqual(buildTargets["linux-arm"], { goos: "linux", goarch: "arm", goarm: "6" })
})

test("derives platform package artifact paths", () => {
  assert.deepEqual(componentArtifact("/repo", "linux-x64", "typescript", "7.1.0", "tsc"), {
    binaryName: "tsc",
    path: "/repo/_packages/tsgo-linux-x64/artifacts/typescript/7.1.0/tsc"
  })
  assert.deepEqual(componentArtifact("/repo", "win32-x64", "typescript", "7.0.0", "tsc"), {
    binaryName: "tsc",
    path: "/repo/_packages/tsgo-win32-x64/artifacts/typescript/7.0.0/tsc.exe"
  })
})

test("maps supported Oxlint release targets", () => {
  assert.deepEqual(oxlintBuildTargets["linux-arm64"], {
    goos: "linux",
    goarch: "arm64",
    rustTarget: "aarch64-unknown-linux-gnu",
    codeTarget: "linux-arm64-gnu",
    napiArgs: ["--use-napi-cross"]
  })
  assert.equal("linux-arm" in oxlintBuildTargets, false)
})

test("rejects unsupported component release targets", async() => {
  await assert.rejects(
    Effect.runPromise(
      buildArtifact("/repo", "oxlint", "1.0.0", "linux-arm").pipe(Effect.provide(NodeServices.layer))
    ),
    /oxlint does not support target linux-arm/
  )
})

test("derives packaged Oxlint artifact paths", () => {
  assert.deepEqual(oxlintArtifacts("/repo", "linux-x64", "1.77.0", "7.0.2001"), {
    packageDirectory: "/repo/_packages/tsgo-linux-x64/artifacts",
    bindingName: "oxlint.node",
    bindingSourceName: "oxlint.linux-x64-gnu.node",
    bindingPath: "/repo/_packages/tsgo-linux-x64/artifacts/oxlint/1.77.0/oxlint.node",
    tsgolintPath: "/repo/_packages/tsgo-linux-x64/artifacts/oxlint-tsgolint/7.0.2001/tsgolint"
  })
  assert.deepEqual(oxlintArtifacts("/repo", "win32-arm64", "1.77.0", "7.0.2001"), {
    packageDirectory: "/repo/_packages/tsgo-win32-arm64/artifacts",
    bindingName: "oxlint.node",
    bindingSourceName: "oxlint.win32-arm64-msvc.node",
    bindingPath: "/repo/_packages/tsgo-win32-arm64/artifacts/oxlint/1.77.0/oxlint.node",
    tsgolintPath: "/repo/_packages/tsgo-win32-arm64/artifacts/oxlint-tsgolint/7.0.2001/tsgolint.exe"
  })
})
