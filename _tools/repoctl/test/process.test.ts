import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import test from "node:test"
import { isWindowsCommandShim, resolveCommand, runCommandString } from "../src/process.ts"

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
