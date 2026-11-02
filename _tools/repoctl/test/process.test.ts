import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import test from "node:test"
import { isWindowsCommandShim, makeCommand, resolveCommand, runCommandString } from "../src/process.ts"

test("resolves Node command shims on Windows", () => {
  assert.equal(resolveCommand("corepack", "win32"), "corepack.cmd")
  assert.equal(resolveCommand("npm", "win32"), "npm.cmd")
  assert.equal(resolveCommand("pnpm", "win32"), "pnpm.cmd")
  assert.equal(resolveCommand("npm", "linux"), "npm")
  assert.equal(resolveCommand("git", "win32"), "git")
})

test("uses a shell for Node command shims on Windows", () => {
  assert.equal(isWindowsCommandShim("corepack", "win32"), true)
  assert.equal(isWindowsCommandShim("npm", "win32"), true)
  assert.equal(isWindowsCommandShim("pnpm", "win32"), true)
  assert.equal(isWindowsCommandShim("npm", "linux"), false)
  assert.equal(isWindowsCommandShim("git", "win32"), false)
})

test("folds arguments into the shell command line for Windows shims", () => {
  const shim = makeCommand("pnpm", ["install", "--frozen-lockfile"], { cwd: "." }, "win32")
  assert.equal(shim._tag, "StandardCommand")
  if (shim._tag === "StandardCommand") {
    assert.equal(shim.command, "pnpm.cmd install --frozen-lockfile")
    assert.deepEqual(shim.args, [])
    assert.equal(shim.options.shell, true)
  }
  const direct = makeCommand("git", ["status"], { cwd: "." }, "win32")
  if (direct._tag === "StandardCommand") {
    assert.equal(direct.command, "git")
    assert.deepEqual(direct.args, ["status"])
    assert.equal(direct.options.shell, undefined)
  }
})

test("captures successful command output", async() => {
  const output = await Effect.runPromise(
    runCommandString(process.execPath, process.cwd(), ["-e", "process.stdout.write('output')"]).pipe(
      Effect.provide(NodeServices.layer)
    )
  )
  assert.equal(output, "output")
})

test("rejects failed output commands", async() => {
  await assert.rejects(
    Effect.runPromise(
      runCommandString(process.execPath, process.cwd(), [
        "-e",
        "process.stderr.write('failure'); process.exit(7)"
      ]).pipe(Effect.provide(NodeServices.layer))
    ),
    /exited with code 7:\nfailure/
  )
})
