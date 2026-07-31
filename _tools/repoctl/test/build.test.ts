import assert from "node:assert/strict"
import test from "node:test"
import { binaryArtifact, buildTargets, isBinaryName } from "../src/build.ts"

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
