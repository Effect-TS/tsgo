import assert from "node:assert/strict"
import test from "node:test"
import { binaryArtifact, buildTargets, isBinaryName, oxlintArtifacts, oxlintBuildTargets } from "../src/build.ts"

test("maps release targets to Go platforms", () => {
  assert.deepEqual(buildTargets["darwin-x64"], { goos: "darwin", goarch: "amd64" })
  assert.deepEqual(buildTargets["win32-arm64"], { goos: "windows", goarch: "arm64" })
  assert.deepEqual(buildTargets["linux-arm"], { goos: "linux", goarch: "arm", goarm: "6" })
})

test("derives platform package artifact paths", () => {
  assert.deepEqual(binaryArtifact("/repo", "linux-x64", "tsc-next"), {
    binaryName: "tsc-next",
    path: "/repo/_packages/tsgo-linux-x64/lib/tsc-next"
  })
  assert.deepEqual(binaryArtifact("/repo", "win32-x64", "tsc"), {
    binaryName: "tsc",
    path: "/repo/_packages/tsgo-win32-x64/lib/tsc.exe"
  })
})

test("validates release binary names", () => {
  assert.equal(isBinaryName("tsc-next"), true)
  assert.equal(isBinaryName("tsc.exe"), true)
  assert.equal(isBinaryName("../tsc"), false)
  assert.equal(isBinaryName("nested/tsc"), false)
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

test("derives packaged Oxlint artifact paths", () => {
  assert.deepEqual(oxlintArtifacts("/repo", "linux-x64"), {
    packageDirectory: "/repo/_packages/tsgo-linux-x64/lib",
    bindingName: "oxlint.linux-x64-gnu.node",
    bindingSourceName: "oxlint.linux-x64-gnu.node",
    bindingPath: "/repo/_packages/tsgo-linux-x64/lib/oxlint.linux-x64-gnu.node",
    tsgolintPath: "/repo/_packages/tsgo-linux-x64/lib/tsgolint"
  })
  assert.deepEqual(oxlintArtifacts("/repo", "win32-arm64"), {
    packageDirectory: "/repo/_packages/tsgo-win32-arm64/lib",
    bindingName: "oxlint.win32-arm64.node",
    bindingSourceName: "oxlint.win32-arm64-msvc.node",
    bindingPath: "/repo/_packages/tsgo-win32-arm64/lib/oxlint.win32-arm64.node",
    tsgolintPath: "/repo/_packages/tsgo-win32-arm64/lib/tsgolint.exe"
  })
})
