import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import test from "node:test"
import {
  buildUpstream,
  decodeLatestNpmVersion,
  decodeUpstream,
  findTypeScriptVersion,
  formatOxlintConfigurationSchema,
  formatTSConfigSchema,
  getComponent
} from "../src/upstream.ts"
import { resolveUpstreamInfo } from "../src/upstreamResolve.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"
const secondRevision = "1123456789abcdef0123456789abcdef01234567"
const thirdRevision = "2123456789abcdef0123456789abcdef01234567"

const manifest = () => ({
  schemaVersion: 4,
  tags: {
    typescript: {
      latest: "7.0.0",
      next: "7.1.0"
    },
    oxlint: { latest: "1.1.0" },
    "oxlint-tsgolint": { latest: "7.0.1000" }
  },
  components: {
    typescript: {
      "7.0.0": { gitHead: revision },
      "7.1.0": { gitHead: secondRevision }
    },
    "oxlint-tsgolint": {
      "7.0.1000": {
        gitHead: revision,
        dependencies: { typescript: "7.0.0" }
      }
    },
    oxlint: {
      "1.0.0": { gitHead: revision },
      "1.1.0": { gitHead: secondRevision }
    }
  },
  profiles: [
    {
      name: "vite-plus",
      description: "Vite+ 1.0.0 compatibility runtime",
      dependencies: { oxlint: "1.0.0", "oxlint-tsgolint": "7.0.1000" }
    }
  ]
})

test("decodes normalized upstream metadata", async() => {
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify(manifest())))
  assert.equal(upstream.tags.typescript.latest, "7.0.0")
  assert.equal(upstream.profiles[0]?.description, "Vite+ 1.0.0 compatibility runtime")
})

test("rejects the deprecated pre-tag format", async() => {
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify({
      schemaVersion: 3,
      typescript: { latest: "7.0.0", next: "7.1.0" },
      components: {},
      profiles: []
    }))),
    /schemaVersion/
  )
})

test("rejects duplicate profile names", async() => {
  const upstream = manifest()
  upstream.profiles.push({ ...upstream.profiles[0]! })
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify(upstream))),
    /Duplicate upstream profile: vite-plus/
  )
})

test("rejects dangling component references", async() => {
  const upstream = manifest()
  upstream.components["oxlint-tsgolint"]["7.0.1000"]!.dependencies.typescript = "missing"
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify(upstream))),
    /references unknown TypeScript missing/
  )
})

test("rejects dangling component tags", async() => {
  const upstream = manifest()
  upstream.tags.oxlint.latest = "missing"
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify(upstream))),
    /oxlint latest references unknown version missing/
  )
})

test("requires runtime profiles to declare both patched components", async() => {
  const upstream = manifest()
  delete (upstream.profiles[0]!.dependencies as Record<string, string>)["oxlint-tsgolint"]
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify(upstream))),
    /Profile vite-plus must depend on oxlint-tsgolint/
  )
})

test("resolves component checkouts and defaults to TypeScript next", async() => {
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify(manifest())))

  assert.deepEqual(await Effect.runPromise(getComponent(upstream, "typescript")), {
    name: "typescript",
    version: "7.1.0",
    gitHead: secondRevision,
    typescript: { version: "7.1.0", gitHead: secondRevision }
  })
  assert.deepEqual(await Effect.runPromise(getComponent(upstream, "oxlint-tsgolint", "7.0.1000")), {
    name: "oxlint-tsgolint",
    version: "7.0.1000",
    gitHead: revision,
    typescript: { version: "7.0.0", gitHead: revision }
  })
  assert.deepEqual(await Effect.runPromise(getComponent(upstream, "oxlint", "1.0.0")), {
    name: "oxlint",
    version: "1.0.0",
    gitHead: revision,
    typescript: { version: "7.1.0", gitHead: secondRevision }
  })
  await assert.rejects(
    Effect.runPromise(getComponent(upstream, "oxlint")),
    /A version is required for component oxlint/
  )
})

test("resolves platform build metadata for setup actions", async() => {
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify(manifest())))

  assert.deepEqual(await Effect.runPromise(resolveUpstreamInfo(upstream, "typescript")), {
    component: "typescript",
    version: "7.1.0"
  })
  assert.deepEqual(await Effect.runPromise(resolveUpstreamInfo(upstream, "oxlint", "1.1.0", "linux-x64")), {
    component: "oxlint",
    version: "1.1.0",
    target: "linux-x64",
    rustTarget: "x86_64-unknown-linux-gnu"
  })
  await assert.rejects(
    Effect.runPromise(resolveUpstreamInfo(upstream, "oxlint", "1.1.0", "linux-arm")),
    /oxlint does not support target linux-arm/
  )
})

test("selects the latest npm version matching a dependency spec regardless of result order", async() => {
  assert.equal(await Effect.runPromise(decodeLatestNpmVersion('"1.0.0"', "package@=1.0.0", "=1.0.0")), "1.0.0")
  assert.equal(
    await Effect.runPromise(decodeLatestNpmVersion(
      '["1.10.0","1.0.0","1.2.0"]',
      "package@^1.0.0",
      "^1.0.0"
    )),
    "1.10.0"
  )
  await assert.rejects(
    Effect.runPromise(decodeLatestNpmVersion("[]", "package@^1.0.0", "^1.0.0")),
    /Unable to resolve package@\^1.0.0/
  )
})

test("builds deterministic normalized metadata and deduplicates components", () => {
  const upstream = buildUpstream({
    next: { npmVersion: "7.1.0", gitHead: secondRevision },
    latest: { npmVersion: "7.0.0", gitHead: revision },
    oxlint: {
      oxlint: { npmVersion: "1.77.0", gitHead: thirdRevision },
      tsgolint: { npmVersion: "7.0.2001", gitHead: secondRevision },
      ts: { npmVersion: "7.0.0", gitHead: revision }
    },
    vitePlus: {
      vitePlusVersion: "0.2.8",
      oxlint: { npmVersion: "1.76.0", gitHead: secondRevision },
      tsgolint: { npmVersion: "7.0.2001", gitHead: secondRevision },
      ts: { npmVersion: "7.0.0", gitHead: revision }
    }
  })

  assert.deepEqual(Object.keys(upstream.components.typescript), ["7.0.0", "7.1.0"])
  assert.deepEqual(Object.keys(upstream.components["oxlint-tsgolint"]), ["7.0.2001"])
  assert.deepEqual(Object.keys(upstream.components.oxlint), ["1.76.0", "1.77.0"])
  assert.deepEqual(upstream.tags, {
    typescript: { latest: "7.0.0", next: "7.1.0" },
    oxlint: { latest: "1.77.0" },
    "oxlint-tsgolint": { latest: "7.0.2001" }
  })
  assert.deepEqual(upstream.profiles, [
    {
      name: "vite-plus",
      description: "Vite+ 0.2.8 compatibility runtime",
      dependencies: { oxlint: "1.76.0", "oxlint-tsgolint": "7.0.2001" }
    }
  ])
})

test("finds a TypeScript npm version by its git head", () => {
  assert.equal(findTypeScriptVersion({
    "7.0.1": { gitHead: revision },
    "7.0.2": { gitHead: secondRevision }
  }, secondRevision), "7.0.2")
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
