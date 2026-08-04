import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import test from "node:test"
import {
  decodeUpstream,
  findTypeScriptVersion,
  formatOxlintConfigurationSchema,
  formatTSConfigSchema,
  getProfile,
  updateOxlintProfile,
  updateTypeScriptProfiles
} from "../src/upstream.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"

test("decodes and looks up typed profiles", async() => {
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify({
    schemaVersion: 2,
    profiles: [
      {
        kind: "ts",
        name: "next",
        ts: { npmVersion: "7.1.0", gitHead: revision },
        binName: "tsc-next"
      },
      {
        kind: "ts",
        name: "latest",
        ts: { npmVersion: "7.0.0", gitHead: revision },
        binName: "tsc"
      },
      {
        kind: "oxlint",
        name: "oxlint",
        ts: { npmVersion: "7.0.0", gitHead: revision },
        tsgolint: { npmVersion: "1.0.0", gitHead: revision },
        oxlint: { npmVersion: "1.0.0", gitHead: revision }
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
    ts: { npmVersion: "7.1.0", gitHead: revision },
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
        ts: { npmVersion: "7.1.0", gitHead: revision },
        binName: "tsc-next"
      },
      {
        kind: "ts",
        name: "latest",
        ts: { npmVersion: "7.0.0", gitHead: revision },
        binName: "tsc"
      },
      {
        kind: "oxlint",
        name: "oxlint",
        ts: { npmVersion: "7.0.0", gitHead: revision },
        tsgolint: { npmVersion: "1.0.0", gitHead: revision },
        oxlint: { npmVersion: "1.0.0", gitHead: revision }
      }
    ]
  })))
  const nextRevision = "1123456789abcdef0123456789abcdef01234567"
  const latestRevision = "2123456789abcdef0123456789abcdef01234567"

  const updated = updateTypeScriptProfiles(upstream, {
    next: { npmVersion: "7.2.0", gitHead: nextRevision },
    latest: { npmVersion: "7.1.0", gitHead: latestRevision }
  })

  assert.deepEqual(updated.profiles[0]?.ts, { npmVersion: "7.2.0", gitHead: nextRevision })
  assert.deepEqual(updated.profiles[1]?.ts, { npmVersion: "7.1.0", gitHead: latestRevision })
  assert.deepEqual(updated.profiles[2]?.ts, upstream.profiles[2]?.ts)
})

test("updates the Oxlint profile as one compatible release chain", async() => {
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify({
    schemaVersion: 2,
    profiles: [
      {
        kind: "ts",
        name: "next",
        ts: { npmVersion: "7.1.0", gitHead: revision },
        binName: "tsc-next"
      },
      {
        kind: "ts",
        name: "latest",
        ts: { npmVersion: "7.0.0", gitHead: revision },
        binName: "tsc"
      },
      {
        kind: "oxlint",
        name: "oxlint",
        ts: { npmVersion: "7.0.0-dev.20260123.3", gitHead: revision },
        tsgolint: { npmVersion: "0.11.2", gitHead: revision },
        oxlint: { npmVersion: "1.42.0", gitHead: revision }
      }
    ]
  })))
  const oxlintRevision = "1123456789abcdef0123456789abcdef01234567"
  const tsgolintRevision = "2123456789abcdef0123456789abcdef01234567"
  const typescriptRevision = "3123456789abcdef0123456789abcdef01234567"

  const updated = updateOxlintProfile(upstream, {
    oxlint: { npmVersion: "1.76.0", gitHead: oxlintRevision },
    tsgolint: { npmVersion: "7.0.2001", gitHead: tsgolintRevision },
    ts: { npmVersion: "7.0.2", gitHead: typescriptRevision }
  })

  assert.deepEqual(updated.profiles[2], {
    kind: "oxlint",
    name: "oxlint",
    oxlint: { npmVersion: "1.76.0", gitHead: oxlintRevision },
    tsgolint: { npmVersion: "7.0.2001", gitHead: tsgolintRevision },
    ts: { npmVersion: "7.0.2", gitHead: typescriptRevision }
  })
})

test("finds a TypeScript npm version by its git head", () => {
  assert.equal(findTypeScriptVersion({
    "7.0.1": { gitHead: revision },
    "7.0.2": { gitHead: "1123456789abcdef0123456789abcdef01234567" }
  }, "1123456789abcdef0123456789abcdef01234567"), "7.0.2")
})

test("formats a JSON Schema Store tsconfig schema", () => {
  assert.equal(formatTSConfigSchema({ definitions: {}, title: "tsconfig" }), [
    "{",
    '  "definitions": {},',
    '  "title": "tsconfig"',
    "}",
    ""
  ].join("\n"))
  assert.equal(formatTSConfigSchema({ title: "not a tsconfig schema" }), undefined)
})

test("formats an Oxlint configuration schema", () => {
  assert.equal(formatOxlintConfigurationSchema({ definitions: {}, properties: {}, title: "Oxlintrc" }), [
    "{",
    '  "definitions": {},',
    '  "properties": {},',
    '  "title": "Oxlintrc"',
    "}",
    ""
  ].join("\n"))
  assert.equal(formatOxlintConfigurationSchema({ definitions: {} }), undefined)
})
