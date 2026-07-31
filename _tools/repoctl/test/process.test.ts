import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import test from "node:test"
import { runCommandString } from "../src/process.ts"

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
