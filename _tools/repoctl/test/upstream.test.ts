import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import test from "node:test"
import {
  buildUpstream,
  decodeLatestNpmVersion,
  decodeUpstream,
  findTypeScriptVersion,
  formatGitHubOutputs,
  formatOxlintConfigurationSchema,
  formatTSConfigSchema,
  formatUpstreamUpdateDescription,
  getComponent
} from "../src/upstream.ts"
import { resolveUpstreamInfo } from "../src/upstreamResolve.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"
const secondRevision = "1123456789abcdef0123456789abcdef01234567"
const thirdRevision = "2123456789abcdef0123456789abcdef01234567"

const manifest = () => ({
  schemaVersion: 5 as const,
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
      "7.0.0": { gitHead: revision, provider: "typescript-go" as const },
      "7.1.0": { gitHead: secondRevision, provider: "typescript" as const }
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
    typescript: {
      version: "7.1.0",
      gitHead: secondRevision,
      source: {
        provider: "typescript",
        repository: "https://github.com/microsoft/TypeScript.git",
        repositorySlug: "microsoft/TypeScript",
        checkoutDir: "typescript",
        moduleDir: "tsc",
        modulePrefix: "github.com/microsoft/TypeScript/tsc",
        providerShimPrefix: "github.com/microsoft/TypeScript/tsc/shim",
        shimOverlayDir: "_tools/gen_shims/providers/typescript",
        patchDir: "_patches/typescript",
        commandPath: "cmd/tsc",
        tsgolintGitlink: "typescript"
      }
    }
  })
  assert.deepEqual(await Effect.runPromise(getComponent(upstream, "oxlint-tsgolint", "7.0.1000")), {
    name: "oxlint-tsgolint",
    version: "7.0.1000",
    gitHead: revision,
    typescript: {
      version: "7.0.0",
      gitHead: revision,
      source: {
        provider: "typescript-go",
        repository: "https://github.com/microsoft/typescript-go.git",
        repositorySlug: "microsoft/typescript-go",
        checkoutDir: "typescript-go",
        moduleDir: ".",
        modulePrefix: "github.com/microsoft/typescript-go",
        providerShimPrefix: "github.com/microsoft/typescript-go/shim",
        shimOverlayDir: "_tools/gen_shims/providers/typescript-go",
        patchDir: "_patches/typescript-go",
        commandPath: "cmd/tsgo",
        tsgolintGitlink: "typescript-go"
      }
    }
  })
  assert.deepEqual(await Effect.runPromise(getComponent(upstream, "oxlint", "1.0.0")), {
    name: "oxlint",
    version: "1.0.0",
    gitHead: revision,
    typescript: {
      version: "7.1.0",
      gitHead: secondRevision,
      source: {
        provider: "typescript",
        repository: "https://github.com/microsoft/TypeScript.git",
        repositorySlug: "microsoft/TypeScript",
        checkoutDir: "typescript",
        moduleDir: "tsc",
        modulePrefix: "github.com/microsoft/TypeScript/tsc",
        providerShimPrefix: "github.com/microsoft/TypeScript/tsc/shim",
        shimOverlayDir: "_tools/gen_shims/providers/typescript",
        patchDir: "_patches/typescript",
        commandPath: "cmd/tsc",
        tsgolintGitlink: "typescript"
      }
    }
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
    next: { npmVersion: "7.1.0", gitHead: secondRevision, provider: "typescript" },
    latest: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" },
    oxlint: {
      oxlint: { npmVersion: "1.77.0", gitHead: thirdRevision },
      tsgolint: { npmVersion: "7.0.2001", gitHead: secondRevision },
      ts: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" }
    },
    vitePlus: {
      vitePlusVersion: "0.2.8",
      oxlint: { npmVersion: "1.76.0", gitHead: secondRevision },
      tsgolint: { npmVersion: "7.0.2001", gitHead: secondRevision },
      ts: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" }
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

test("describes upstream version and TypeScript-Go commit updates", () => {
  const before = manifest()
  const after = manifest()
  const afterTypeScript = after.components.typescript as Record<string, {
    gitHead: string
    provider: "typescript-go" | "typescript"
  }>
  const afterOxlint = after.components.oxlint as Record<string, { gitHead: string }>
  after.tags.typescript.next = "7.2.0"
  afterTypeScript["7.2.0"] = { gitHead: thirdRevision, provider: "typescript" }
  after.tags.oxlint.latest = "1.2.0"
  afterOxlint["1.2.0"] = { gitHead: thirdRevision }
  after.profiles[0]!.description = "Vite+ 1.1.0 compatibility runtime"
  after.profiles[0]!.dependencies.oxlint = "1.2.0"

  assert.equal(formatUpstreamUpdateDescription({
    before,
    after,
    nextCommits: [{ sha: thirdRevision, message: "Add a useful feature" }],
    schemaChanged: true,
    oxlintSchemaChanged: false
  }), [
    "Automated update of upstream metadata, generated TypeScript next-tag sources, and Nix inputs.",
    "",
    "## Version updates",
    "",
    "- TypeScript next: [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.2.0) `7.1.0` -> `7.2.0`",
    "- Oxlint: [`oxlint@latest`](https://www.npmjs.com/package/oxlint/v/1.2.0) `1.1.0` -> `1.2.0`",
    "- Vite+: [`vite-plus@latest`](https://www.npmjs.com/package/vite-plus/v/1.1.0) `1.0.0` -> `1.1.0`",
    "- Vite+ Oxlint runtime: [`oxlint`](https://www.npmjs.com/package/oxlint/v/1.2.0) `1.0.0` -> `1.2.0`",
    "",
    "## TypeScript compiler",
    "",
    `- Previous commit: [\`${secondRevision}\`](https://github.com/microsoft/TypeScript/commit/${secondRevision})`,
    `- Updated commit: [\`${thirdRevision}\`](https://github.com/microsoft/TypeScript/commit/${thirdRevision})`,
    `- Compare: https://github.com/microsoft/TypeScript/compare/${secondRevision}...${thirdRevision}`,
    "",
    "## Upstream commits",
    "",
    `- [${thirdRevision.slice(0, 7)}](https://github.com/microsoft/TypeScript/commit/${thirdRevision}) Add a useful feature`,
    "",
    "## Other updates",
    "",
    "- Refreshed the tsconfig schema from JSON Schema Store."
  ].join("\n"))
})

test("writes multiline descriptions as GitHub step outputs", () => {
  assert.equal(formatGitHubOutputs({ has_changes: "true", description: "first\nsecond" }), [
    "has_changes=true",
    "description<<repoctl_description",
    "first",
    "second",
    "repoctl_description",
    ""
  ].join("\n"))
  assert.equal(formatGitHubOutputs({ description: "repoctl_description\nbody" }), [
    "description<<repoctl_description_",
    "repoctl_description",
    "body",
    "repoctl_description_",
    ""
  ].join("\n"))
  assert.equal(formatGitHubOutputs({ description: "first\rsecond" }), [
    "description<<repoctl_description",
    "first",
    "second",
    "repoctl_description",
    ""
  ].join("\n"))
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
