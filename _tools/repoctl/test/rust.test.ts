import * as NodeServices from "@effect/platform-node/NodeServices"
import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import * as Effect from "effect/Effect"
import { requireTsrsCheckout, runRustChecks, runRustLint, runRustTests, tsrsCheckedOut } from "../src/rust.ts"

const run = <A, E>(effect: Effect.Effect<A, E, NodeServices.NodeServices>) =>
  Effect.runPromise(effect.pipe(Effect.provide(NodeServices.layer)))

test("the tsrs cargo steps fail without a tsrs checkout, before running cargo", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-tsrs-"))
  try {
    assert.equal(await run(tsrsCheckedOut(repository)), false)
    for (const step of [requireTsrsCheckout, runRustChecks, runRustLint, runRustTests]) {
      await assert.rejects(
        run(step(repository)),
        /tsrs\/ is not checked out: run `pnpm exec repoctl submodules setup --component tsrs`/
      )
    }

    mkdirSync(join(repository, "tsrs"))
    writeFileSync(join(repository, "tsrs", "Cargo.toml"), "[workspace]\n")
    assert.equal(await run(tsrsCheckedOut(repository)), true)
    await run(requireTsrsCheckout(repository))
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})
