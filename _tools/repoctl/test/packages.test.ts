import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import assert from "node:assert/strict"
import test from "node:test"
import { buildTargets } from "../src/build.ts"
import { bundleUpstream } from "../src/packages.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"

test("bundles upstream metadata in every platform package", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-packages-"))
  const upstream = `${JSON.stringify({
    schemaVersion: 2,
    profiles: [
      { kind: "ts", name: "next", ts: { version: "7.1.0", gitHead: revision }, binName: "tsc-next" },
      { kind: "ts", name: "latest", ts: { version: "7.0.0", gitHead: revision }, binName: "tsc" },
      {
        kind: "oxlint",
        name: "oxlint",
        ts: { version: "7.0.0", gitHead: revision },
        tsgolint: { version: "1.0.0", gitHead: revision },
        oxlint: { version: "1.0.0", gitHead: revision }
      }
    ]
  }, null, 2)}\n`

  try {
    mkdirSync(join(repository, "_packages", "tsgo"), { recursive: true })
    writeFileSync(join(repository, "_packages", "tsgo", "upstream.json"), upstream)

    await Effect.runPromise(bundleUpstream(repository).pipe(Effect.provide(NodeServices.layer)))

    for (const target of Object.keys(buildTargets)) {
      assert.equal(
        readFileSync(join(repository, "_packages", `tsgo-${target}`, "lib", "upstream.json"), "utf8"),
        upstream
      )
    }
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})
