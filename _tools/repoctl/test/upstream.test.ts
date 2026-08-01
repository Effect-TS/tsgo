import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import test from "node:test"
import { decodeUpstream, getProfile, updateTypeScriptProfiles } from "../src/upstream.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"

test("decodes and looks up typed profiles", async() => {
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify({
    schemaVersion: 2,
    profiles: [
      {
        kind: "ts",
        name: "next",
        ts: { version: "7.1.0", gitHead: revision },
        binName: "tsc-next"
      },
      {
        kind: "ts",
        name: "latest",
        ts: { version: "7.0.0", gitHead: revision },
        binName: "tsc"
      },
      {
        kind: "oxlint",
        name: "oxlint",
        ts: { version: "7.0.0", gitHead: revision },
        tsgolint: { version: "1.0.0", gitHead: revision },
        oxlint: { version: "1.0.0", gitHead: revision }
      }
    ]
  })))

  const next = await Effect.runPromise(getProfile(upstream, "next"))
  assert.equal(next.kind, "ts")
  assert.equal(next.binName, "tsc-next")
})

test("rejects duplicate profile names", async() => {
  const profile = {
    kind: "ts",
    name: "next",
    ts: { version: "7.1.0", gitHead: revision },
    binName: "tsc-next"
  }
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify({
      schemaVersion: 2,
      profiles: [profile, profile]
    }))),
    /Duplicate upstream profile: next/
  )
})

test("updates moving TypeScript profiles", async() => {
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify({
    schemaVersion: 2,
    profiles: [
      {
        kind: "ts",
        name: "next",
        ts: { version: "7.1.0", gitHead: revision },
        binName: "tsc-next"
      },
      {
        kind: "ts",
        name: "latest",
        ts: { version: "7.0.0", gitHead: revision },
        binName: "tsc"
      },
      {
        kind: "oxlint",
        name: "oxlint",
        ts: { version: "7.0.0", gitHead: revision },
        tsgolint: { version: "1.0.0", gitHead: revision },
        oxlint: { version: "1.0.0", gitHead: revision }
      }
    ]
  })))
  const nextRevision = "1123456789abcdef0123456789abcdef01234567"
  const latestRevision = "2123456789abcdef0123456789abcdef01234567"

  const updated = updateTypeScriptProfiles(upstream, {
    next: { version: "7.2.0", gitHead: nextRevision },
    latest: { version: "7.1.0", gitHead: latestRevision }
  })

  assert.deepEqual(updated.profiles[0]?.ts, { version: "7.2.0", gitHead: nextRevision })
  assert.deepEqual(updated.profiles[1]?.ts, { version: "7.1.0", gitHead: latestRevision })
  assert.deepEqual(updated.profiles[2]?.ts, upstream.profiles[2]?.ts)
})
