import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { buildOxlintTestMatrix } from "../src/matrix.ts"
import {
  buildUpstream,
  decodeLatestNpmVersion,
  decodeRecentNpmVersions,
  decodeUpstream,
  findTypeScriptVersion,
  formatGitHubOutputs,
  formatOxlintConfigurationSchema,
  formatTSConfigSchema,
  formatUpstreamUpdateDescription,
  getComponent,
  getTsrsComponent,
  parseTsrsNpmVersion,
  parseTsrsTypeScriptMetadata,
  parseTypeScriptOverrideFolderName,
  resolveComponent,
  resolveMaterializedTypeScriptSource,
  typeScriptSourceEntrypoint
} from "../src/upstream.ts"
import { resolveUpstreamInfo } from "../src/upstreamResolve.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"
const secondRevision = "1123456789abcdef0123456789abcdef01234567"
const thirdRevision = "2123456789abcdef0123456789abcdef01234567"

const resolveIn = (
  repositoryRoot: string,
  upstream: Parameters<typeof resolveComponent>[1],
  name: Parameters<typeof resolveComponent>[2],
  version?: string
) => Effect.runPromise(resolveComponent(repositoryRoot, upstream, name, version).pipe(Effect.provide(NodeServices.layer)))

const withRepository = async(run: (repository: string) => Promise<void>) => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-typescript-overrides-"))
  try {
    await run(repository)
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
}

test("prefers a materialized TSC entrypoint and falls back to TypeScript-Go", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-typescript-source-"))
  const resolve = () => Effect.runPromise(
    resolveMaterializedTypeScriptSource(repository).pipe(Effect.provide(NodeServices.layer))
  )
  try {
    mkdirSync(join(repository, "typescript", "tsc", "cmd", "tsc"), { recursive: true })
    mkdirSync(join(repository, "typescript-go", "cmd", "tsgo"), { recursive: true })

    const modern = await resolve()
    assert.equal(modern.provider, "typescript")
    assert.equal(typeScriptSourceEntrypoint(modern), "./typescript/tsc/cmd/tsc")

    rmSync(join(repository, "typescript"), { recursive: true, force: true })
    const legacy = await resolve()
    assert.equal(legacy.provider, "typescript-go")
    assert.equal(typeScriptSourceEntrypoint(legacy), "./typescript-go/cmd/tsgo")

    rmSync(join(repository, "typescript-go"), { recursive: true, force: true })
    await assert.rejects(resolve(), /neither \.\/typescript\/tsc\/cmd\/tsc nor \.\/typescript-go\/cmd\/tsgo exists/)
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})

const manifest = () => ({
  schemaVersion: 5 as const,
  tags: {
    typescript: {
      latest: "7.0.0",
      beta: "7.1.0",
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

test("decodes manifests that predate the TypeScript beta tag", async() => {
  const upstream = manifest()
  delete (upstream.tags.typescript as { beta?: string }).beta
  const decoded = await Effect.runPromise(decodeUpstream(JSON.stringify(upstream)))
  assert.equal(decoded.tags.typescript.beta, undefined)
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

test("rejects a dangling TypeScript beta tag", async() => {
  const upstream = manifest()
  upstream.tags.typescript.beta = "missing"
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify(upstream))),
    /TypeScript beta references unknown version missing/
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

test("resolves component checkouts and defaults to TypeScript next", () => withRepository(async(repository) => {
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify(manifest())))

  assert.deepEqual(await resolveIn(repository, upstream, "typescript"), {
    name: "typescript",
    version: "7.1.0",
    gitHead: secondRevision,
    typescript: {
      version: "7.1.0",
      gitHead: secondRevision,
      provider: "typescript",
      source: {
        provider: "typescript",
        repository: "https://github.com/microsoft/TypeScript.git",
        repositorySlug: "microsoft/TypeScript",
        checkoutDir: "typescript",
        moduleDir: "tsc",
        modulePrefix: "github.com/microsoft/TypeScript/tsc",
        providerShimPrefix: "github.com/microsoft/TypeScript/tsc/shim",
        shimOverlayDirs: ["_tools/gen_shims/providers/typescript"],
        patchDir: "_patches/typescript",
        commandPath: "cmd/tsc",
        tsgolintGitlink: "typescript"
      }
    }
  })
  assert.deepEqual(await resolveIn(repository, upstream, "oxlint-tsgolint", "7.0.1000"), {
    name: "oxlint-tsgolint",
    version: "7.0.1000",
    gitHead: revision,
    typescript: {
      version: "7.0.0",
      gitHead: revision,
      provider: "typescript-go",
      source: {
        provider: "typescript-go",
        repository: "https://github.com/microsoft/typescript-go.git",
        repositorySlug: "microsoft/typescript-go",
        checkoutDir: "typescript-go",
        moduleDir: ".",
        modulePrefix: "github.com/microsoft/typescript-go",
        providerShimPrefix: "github.com/microsoft/typescript-go/shim",
        shimOverlayDirs: ["_tools/gen_shims/providers/typescript-go"],
        patchDir: "_patches/typescript-go",
        commandPath: "cmd/tsgo",
        tsgolintGitlink: "typescript-go"
      }
    }
  })
  assert.deepEqual(await resolveIn(repository, upstream, "oxlint", "1.0.0"), {
    name: "oxlint",
    version: "1.0.0",
    gitHead: revision,
    typescript: {
      version: "7.1.0",
      gitHead: secondRevision,
      provider: "typescript",
      source: {
        provider: "typescript",
        repository: "https://github.com/microsoft/TypeScript.git",
        repositorySlug: "microsoft/TypeScript",
        checkoutDir: "typescript",
        moduleDir: "tsc",
        modulePrefix: "github.com/microsoft/TypeScript/tsc",
        providerShimPrefix: "github.com/microsoft/TypeScript/tsc/shim",
        shimOverlayDirs: ["_tools/gen_shims/providers/typescript"],
        patchDir: "_patches/typescript",
        commandPath: "cmd/tsc",
        tsgolintGitlink: "typescript"
      }
    }
  })
  await assert.rejects(
    resolveIn(repository, upstream, "oxlint"),
    /A version is required for component oxlint/
  )
  // getComponent leaves the source (and its override folders) to resolveComponent.
  assert.deepEqual((await Effect.runPromise(getComponent(upstream, "typescript"))).typescript, {
    version: "7.1.0",
    gitHead: secondRevision,
    provider: "typescript"
  })
}))

test("resolves platform build metadata for setup actions", async() => {
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify(manifest())))

  assert.deepEqual(await Effect.runPromise(resolveUpstreamInfo(upstream, "typescript")), {
    component: "typescript",
    version: "7.1.0",
    goCache: { component: "typescript", version: "7.1.0", save: true }
  })
  assert.deepEqual(await Effect.runPromise(resolveUpstreamInfo(upstream, "oxlint-tsgolint", "7.0.1000")), {
    component: "oxlint-tsgolint",
    version: "7.0.1000",
    goCache: { component: "typescript", version: "7.0.0", save: false }
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

test("retains the three newest stable npm versions through the latest tag", async() => {
  const decode = (versions: Array<string>, latest: string) => Effect.runPromise(decodeRecentNpmVersions(
    JSON.stringify({ versions, "dist-tags.latest": latest }), "oxlint"
  ))
  assert.deepEqual(await decode([
    "1.8.0", "1.10.0", "1.9.0", "1.7.0", "2.0.0-beta.1", "2.0.0", "1.10.0"
  ], "1.10.0"), ["1.10.0", "1.9.0", "1.8.0"])
  assert.deepEqual(await decode(["0.2.0", "0.1.0"], "0.2.0"), ["0.2.0", "0.1.0"])
  await assert.rejects(decode([], "1.0.0"), /No stable latest version found/)
})

test("builds deterministic normalized metadata and deduplicates components", () => {
  const upstream = buildUpstream({
    next: { npmVersion: "7.1.0", gitHead: secondRevision, provider: "typescript" },
    beta: { npmVersion: "7.1.0-beta", gitHead: secondRevision, provider: "typescript" },
    latest: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" },
    oxlint: {
      oxlint: { npmVersion: "1.77.0", gitHead: thirdRevision },
      tsgolint: { npmVersion: "7.0.2001", gitHead: secondRevision },
      ts: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" }
    },
    retainedRuntimes: ["1.72.0", "1.73.0", "1.74.0", "1.75.0", "1.76.0", "1.77.0"].map((npmVersion) => ({
      name: `oxlint@${npmVersion}`,
      description: `Oxlint ${npmVersion} compatibility runtime`,
      oxlint: { npmVersion, gitHead: thirdRevision },
      tsgolint: { npmVersion: npmVersion < "1.76.0" ? "7.0.2000" : "7.0.2001", gitHead: secondRevision },
      ts: { npmVersion: npmVersion < "1.76.0" ? "6.0.0" : "7.0.0", gitHead: revision, provider: "typescript-go" }
    })),
    vitePlus: {
      vitePlusVersion: "0.2.8",
      oxlint: { npmVersion: "1.76.0", gitHead: secondRevision },
      tsgolint: { npmVersion: "7.0.2001", gitHead: secondRevision },
      ts: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" }
    }
  })

  assert.deepEqual(Object.keys(upstream.components.typescript), ["6.0.0", "7.0.0", "7.1.0", "7.1.0-beta"])
  assert.deepEqual(Object.keys(upstream.components["oxlint-tsgolint"]), ["7.0.2000", "7.0.2001"])
  assert.deepEqual(Object.keys(upstream.components.oxlint), ["1.72.0", "1.73.0", "1.74.0", "1.75.0", "1.76.0", "1.77.0"])
  assert.deepEqual(upstream.components["oxlint-tsgolint"]["7.0.2000"]!.dependencies, { typescript: "6.0.0" })
  assert.deepEqual(upstream.tags, {
    typescript: { latest: "7.0.0", beta: "7.1.0-beta", next: "7.1.0" },
    oxlint: { latest: "1.77.0" },
    "oxlint-tsgolint": { latest: "7.0.2001" }
  })
  assert.equal(buildOxlintTestMatrix(upstream).include.length, 6)
  assert.deepEqual(upstream.profiles.slice(1).map((profile) => profile.dependencies.oxlint), [
    "1.72.0", "1.73.0", "1.74.0", "1.75.0", "1.76.0", "1.77.0"
  ])
  assert.deepEqual(upstream.profiles.slice(0, 1), [
    {
      name: "vite-plus",
      description: "Vite+ 0.2.8 compatibility runtime",
      dependencies: { oxlint: "1.76.0", "oxlint-tsgolint": "7.0.2001" }
    }
  ])
})

test("only a provider name followed by a full commit makes an override folder", () => {
  assert.deepEqual(parseTypeScriptOverrideFolderName(`typescript-${revision}`), { provider: "typescript", gitHead: revision })
  assert.deepEqual(parseTypeScriptOverrideFolderName(`typescript-go-${revision}`), {
    provider: "typescript-go",
    gitHead: revision
  })
  // The legacy provider's own folder, the TypeScript provider's, and names that are no full lowercase commit.
  for (const name of [
    "typescript-go",
    "typescript",
    "tsrs",
    "typescript-beta",
    "typescript-tsrs",
    `typescript-${revision.slice(1)}`,
    `typescript-${revision}0`,
    `typescript-${revision.toUpperCase()}`,
    `tsrs-${revision}`,
    `typescript-go-go-${revision}`
  ]) {
    assert.equal(parseTypeScriptOverrideFolderName(name), undefined, name)
  }
})

test("replaces the provider's patch folder and shim overlays with the override folders of a commit", () =>
  withRepository(async(repository) => {
    const base = manifest()
    const tsrsVersion = "0.18.0-ts7.1.0-dev.20260929"
    const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify({
      ...base,
      tags: { ...base.tags, typescript: { ...base.tags.typescript, beta: "7.1.0-beta" }, tsrs: { latest: tsrsVersion } },
      components: {
        ...base.components,
        typescript: {
          ...base.components.typescript,
          "7.1.0-beta": { gitHead: thirdRevision, provider: "typescript" }
        },
        tsrs: { [tsrsVersion]: { gitHead: revision, typescript: { version: "7.1.0-beta", gitHead: thirdRevision } } }
      }
    })))
    const source = async(version: string) => (await resolveIn(repository, upstream, "typescript", version)).typescript.source
    const provider = {
      providerShimPrefix: "github.com/microsoft/TypeScript/tsc/shim",
      shimOverlayDirs: ["_tools/gen_shims/providers/typescript"],
      patchDir: "_patches/typescript"
    }
    const pick = ({ patchDir, providerShimPrefix, shimOverlayDirs }: Awaited<ReturnType<typeof source>>) => ({
      providerShimPrefix,
      shimOverlayDirs,
      patchDir
    })

    // Without override folders, every commit uses the provider's.
    assert.deepEqual(pick(await source("7.1.0-beta")), provider)

    // A shim folder alone: the only overlay (self-contained), through the backport prefix; the patches stay.
    const shims = `_tools/gen_shims/providers/typescript-${thirdRevision}`
    mkdirSync(join(repository, shims, "tsoptions"), { recursive: true })
    assert.deepEqual(pick(await source("7.1.0-beta")), {
      providerShimPrefix: "github.com/microsoft/TypeScript/tsc/providershim",
      shimOverlayDirs: [shims],
      patchDir: "_patches/typescript"
    })

    // With a patch folder too, applied instead of _patches/typescript.
    const patches = `_patches/typescript-${thirdRevision}`
    mkdirSync(join(repository, patches), { recursive: true })
    const overridden = {
      providerShimPrefix: "github.com/microsoft/TypeScript/tsc/providershim",
      shimOverlayDirs: [shims],
      patchDir: patches
    }
    assert.deepEqual(pick(await source("7.1.0-beta")), overridden)
    // The tsrs component resolves the TypeScript it ports the same way.
    assert.deepEqual(pick((await resolveIn(repository, upstream, "tsrs")).typescript.source), overridden)

    // Other commits keep the provider's folders.
    assert.deepEqual(pick(await source("7.1.0")), provider)

    // The legacy provider: its own folder is `typescript-go`; an override keeps its (already backporting) prefix.
    mkdirSync(join(repository, "_patches", "typescript-go"), { recursive: true })
    mkdirSync(join(repository, "_patches", `typescript-go-${revision}`), { recursive: true })
    mkdirSync(join(repository, "_tools", "gen_shims", "providers", `typescript-go-${revision}`), { recursive: true })
    assert.deepEqual(pick(await source("7.0.0")), {
      providerShimPrefix: "github.com/microsoft/typescript-go/shim",
      shimOverlayDirs: [`_tools/gen_shims/providers/typescript-go-${revision}`],
      patchDir: `_patches/typescript-go-${revision}`
    })
    // A folder naming the commit under the other provider does not apply.
    mkdirSync(join(repository, "_patches", `typescript-go-${secondRevision}`), { recursive: true })
    assert.equal((await source("7.1.0")).patchDir, "_patches/typescript")
  }))

test("keeps the TypeScript the pinned tsrs ports through updates", async() => {
  const base = manifest()
  const tsrsVersion = "0.18.0-ts7.1.0-dev.20260929"
  const ported = { gitHead: thirdRevision, provider: "typescript" as const }
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify({
    ...base,
    tags: { ...base.tags, tsrs: { latest: tsrsVersion } },
    components: {
      ...base.components,
      typescript: { ...base.components.typescript, "7.1.0-dev.20260929": ported },
      tsrs: {
        [tsrsVersion]: { gitHead: secondRevision, typescript: { version: "7.1.0-dev.20260929", gitHead: thirdRevision } }
      }
    }
  })))
  const runtime = {
    oxlint: { npmVersion: "1.77.0", gitHead: thirdRevision },
    tsgolint: { npmVersion: "7.0.2001", gitHead: secondRevision },
    ts: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" as const }
  }
  const rebuilt = buildUpstream({
    next: { npmVersion: "7.1.0", gitHead: secondRevision, provider: "typescript" },
    beta: { npmVersion: "7.1.0", gitHead: secondRevision, provider: "typescript" },
    latest: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" },
    oxlint: runtime,
    vitePlus: { ...runtime, vitePlusVersion: "0.2.8" },
    tsrs: { tags: upstream.tags.tsrs!, components: upstream.components.tsrs! }
  })
  assert.deepEqual(rebuilt.components.typescript["7.1.0-dev.20260929"], ported)
  await Effect.runPromise(decodeUpstream(JSON.stringify(rebuilt)))
})

test("resolves the tsrs component and preserves it through updates", async() => {
  const base = manifest()
  const tsrsVersion = "0.18.0-ts7.1.0-dev.20260929"
  const tsrsComponent = {
    gitHead: thirdRevision,
    typescript: { version: "7.1.0-dev.20260929", gitHead: secondRevision }
  }
  const withTsrs = {
    ...base,
    tags: { ...base.tags, tsrs: { latest: tsrsVersion } },
    components: {
      ...base.components,
      typescript: {
        ...base.components.typescript,
        "7.1.0-dev.20260929": { gitHead: secondRevision, provider: "typescript" as const }
      },
      tsrs: { [tsrsVersion]: tsrsComponent }
    }
  }
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify(withTsrs)))
  assert.deepEqual(await Effect.runPromise(getTsrsComponent(upstream)), {
    name: "tsrs",
    version: tsrsVersion,
    gitHead: thirdRevision,
    typescript: tsrsComponent.typescript
  })
  await assert.rejects(Effect.runPromise(getTsrsComponent(upstream, "0.0.0")), /Unknown tsrs component version: 0\.0\.0/)
  const without = await Effect.runPromise(decodeUpstream(JSON.stringify(base)))
  assert.equal(await Effect.runPromise(getTsrsComponent(without)), undefined)

  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify({ ...withTsrs, tags: { ...withTsrs.tags, tsrs: { latest: "0.0.0" } } }))),
    /tsrs latest references unknown version 0\.0\.0/
  )
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify({ ...withTsrs, tags: base.tags }))),
    /tsrs must have both a latest tag and components, or neither/
  )
  // The TypeScript a tsrs component ports must be recorded, on microsoft/TypeScript at the same commit.
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify({
      ...withTsrs,
      components: { ...withTsrs.components, typescript: base.components.typescript }
    }))),
    /tsrs 0\.18\.0-ts7\.1\.0-dev\.20260929 references unknown TypeScript 7\.1\.0-dev\.20260929/
  )
  await assert.rejects(
    Effect.runPromise(decodeUpstream(JSON.stringify({
      ...withTsrs,
      components: {
        ...withTsrs.components,
        typescript: {
          ...base.components.typescript,
          "7.1.0-dev.20260929": { gitHead: revision, provider: "typescript" }
        }
      }
    }))),
    /ports TypeScript 7\.1\.0-dev\.20260929 at [0-9a-f]{40}, but the TypeScript component is typescript [0-9a-f]{40}/
  )

  const runtime = {
    oxlint: { npmVersion: "1.77.0", gitHead: thirdRevision },
    tsgolint: { npmVersion: "7.0.2001", gitHead: secondRevision },
    ts: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" as const }
  }
  const rebuilt = buildUpstream({
    next: { npmVersion: "7.1.0", gitHead: secondRevision, provider: "typescript" },
    beta: { npmVersion: "7.1.0", gitHead: secondRevision, provider: "typescript" },
    latest: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" },
    oxlint: runtime,
    vitePlus: { ...runtime, vitePlusVersion: "0.2.8" },
    tsrs: { tags: upstream.tags.tsrs!, components: upstream.components.tsrs! }
  })
  assert.deepEqual(rebuilt.tags.tsrs, { latest: tsrsVersion })
  assert.deepEqual(rebuilt.components.tsrs, { [tsrsVersion]: tsrsComponent })
  // The TypeScript the recorded tsrs ports stays a TypeScript component (none of the TypeScript tags names it).
  assert.deepEqual(rebuilt.components.typescript["7.1.0-dev.20260929"], { gitHead: secondRevision, provider: "typescript" })
  await Effect.runPromise(decodeUpstream(JSON.stringify(rebuilt)))

  // A tsrs bump switches it: the new tsrs's TypeScript replaces the old one.
  const bumpedVersion = "0.19.0-ts7.1.0-dev.20261101"
  const bumpedComponent = { gitHead: secondRevision, typescript: { version: "7.1.0-dev.20261101", gitHead: thirdRevision } }
  const bumped = buildUpstream({
    next: { npmVersion: "7.1.0", gitHead: secondRevision, provider: "typescript" },
    beta: { npmVersion: "7.1.0", gitHead: secondRevision, provider: "typescript" },
    latest: { npmVersion: "7.0.0", gitHead: revision, provider: "typescript-go" },
    oxlint: runtime,
    vitePlus: { ...runtime, vitePlusVersion: "0.2.8" },
    tsrs: { tags: { latest: bumpedVersion }, components: { [bumpedVersion]: bumpedComponent } }
  })
  assert.deepEqual(Object.keys(bumped.components.typescript), ["7.0.0", "7.1.0", "7.1.0-dev.20261101"])
  assert.deepEqual(bumped.components.typescript["7.1.0-dev.20261101"], { gitHead: thirdRevision, provider: "typescript" })
  await Effect.runPromise(decodeUpstream(JSON.stringify(bumped)))
})

test("resolves the tsrs component next to the TypeScript it ports", async() => {
  const base = manifest()
  const tsrsVersion = "0.18.0-ts7.1.0-dev.20260929"
  const upstream = await Effect.runPromise(decodeUpstream(JSON.stringify({
    ...base,
    tags: { ...base.tags, tsrs: { latest: tsrsVersion } },
    components: {
      ...base.components,
      typescript: { ...base.components.typescript, "7.1.0-dev": { gitHead: thirdRevision, provider: "typescript" } },
      tsrs: { [tsrsVersion]: { gitHead: thirdRevision, typescript: { version: "7.1.0-dev", gitHead: thirdRevision } } }
    }
  })))
  // The TypeScript the tsrs version ports (as oxlint-tsgolint's dependency), not the next tag.
  const selected = await Effect.runPromise(getComponent(upstream, "tsrs", tsrsVersion))
  assert.equal(selected.name, "tsrs")
  assert.equal(selected.gitHead, thirdRevision)
  assert.equal(selected.typescript.version, "7.1.0-dev")
  assert.equal(selected.typescript.gitHead, thirdRevision)
  assert.equal(selected.typescript.provider, "typescript")
  // Without a version: the latest tsrs (`repoctl submodules setup --component tsrs`).
  const latest = await Effect.runPromise(getComponent(upstream, "tsrs"))
  assert.equal(latest.version, tsrsVersion)
  assert.equal(latest.typescript.version, "7.1.0-dev")
  await assert.rejects(
    Effect.runPromise(getComponent(await Effect.runPromise(decodeUpstream(JSON.stringify(base))), "tsrs")),
    /A version is required for component tsrs/
  )
  await assert.rejects(Effect.runPromise(getComponent(upstream, "tsrs", "0.0.0")), /Unknown tsrs component version: 0\.0\.0/)

  assert.deepEqual(await Effect.runPromise(resolveUpstreamInfo(upstream, "tsrs", tsrsVersion, "linux-arm64")), {
    component: "tsrs",
    version: tsrsVersion,
    target: "linux-arm64",
    goCache: { component: "typescript", version: "7.1.0-dev", save: false }
  })
  for (const target of ["win32-arm64", "linux-arm"] as const) {
    await assert.rejects(
      Effect.runPromise(resolveUpstreamInfo(upstream, "tsrs", tsrsVersion, target)),
      new RegExp(`tsrs does not support target ${target}`)
    )
  }
})

test("maps tsrs npm releases to their GitHub tag and ported TypeScript", () => {
  assert.deepEqual(parseTsrsNpmVersion("0.18.0-ts7.1.0-dev.20260929"), {
    tag: "v0.18.0",
    typescriptVersion: "7.1.0-dev.20260929"
  })
  assert.deepEqual(parseTsrsNpmVersion("1.2.3-ts7.1.0"), { tag: "v1.2.3", typescriptVersion: "7.1.0" })
  assert.equal(parseTsrsNpmVersion("0.0.0-snapshot-20240905062806"), undefined)
  assert.equal(parseTsrsNpmVersion("0.18.0"), undefined)

  const cargoToml = [
    "[workspace.package]",
    "version = \"0.18.0\"",
    "",
    "[workspace.metadata.typescript]",
    "version = \"7.1.0-dev.20260929\"",
    `commit = "${secondRevision}" # microsoft/TypeScript`,
    "nightly = \"7.1.0-dev.20260930.4\"",
    "",
    "[workspace.dependencies]",
    "version = \"9.9.9\""
  ].join("\n")
  assert.deepEqual(parseTsrsTypeScriptMetadata(cargoToml), {
    version: "7.1.0-dev.20260929",
    gitHead: secondRevision
  })
  assert.equal(parseTsrsTypeScriptMetadata("[workspace.package]\nversion = \"0.18.0\"\n"), undefined)
  assert.equal(
    parseTsrsTypeScriptMetadata("[workspace.metadata.typescript]\nversion = \"7.1.0\"\ncommit = \"main\"\n"),
    undefined
  )
})

test("describes tsrs updates", () => {
  const before = manifest()
  const tsrs = (version: string, gitHead: string) => ({
    tags: { ...before.tags, tsrs: { latest: version } },
    components: {
      ...before.components,
      tsrs: { [version]: { gitHead, typescript: { version: "7.1.0", gitHead: secondRevision } } }
    }
  })
  const previous = { ...before, ...tsrs("0.17.0-ts7.1.0", revision) }
  const updated = { ...before, ...tsrs("0.18.0-ts7.1.0", thirdRevision) }
  const options = { nextCommits: [], schemaChanged: false, oxlintSchemaChanged: false }
  const description = formatUpstreamUpdateDescription({ ...options, before: previous, after: updated })
  assert.equal(description, [
    "Automated update of upstream metadata, generated TypeScript next-tag sources, and Nix inputs.",
    "",
    "## Version updates",
    "",
    "- tsrs: [`tsrs@latest`](https://www.npmjs.com/package/tsrs/v/0.18.0-ts7.1.0) `0.17.0-ts7.1.0` -> `0.18.0-ts7.1.0`",
    "",
    "## tsrs",
    "",
    `- Previous commit: [\`${revision}\`](https://github.com/oxc-project/tsrs/commit/${revision})`,
    `- Updated commit: [\`${thirdRevision}\`](https://github.com/oxc-project/tsrs/commit/${thirdRevision})`,
    `- Compare: https://github.com/oxc-project/tsrs/compare/${revision}...${thirdRevision}`,
    `- Ports TypeScript \`7.1.0\` ([\`${secondRevision}\`](https://github.com/microsoft/TypeScript/commit/${secondRevision}))`,
    "",
    "The Rust port follows tsrs: `_patches/tsrs` must apply with zero fuzz, and `rust-toolchain.toml` and",
    "`[profile.release]` in `Cargo.toml` must match tsrs's (`sync-upstream` skill)."
  ].join("\n"))

  const added = formatUpstreamUpdateDescription({ ...options, before, after: updated })
  assert.match(added, /`tsrs@latest`.* `none` -> `0\.18\.0-ts7\.1\.0`/)
  assert.doesNotMatch(added, /Previous commit|Compare:/)
  assert.doesNotMatch(
    formatUpstreamUpdateDescription({ ...options, before: updated, after: updated }),
    /tsrs/
  )
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
  after.tags.typescript.beta = "7.2.0"
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
    "- TypeScript beta: [`typescript@beta`](https://www.npmjs.com/package/typescript/v/7.2.0) `7.1.0` -> `7.2.0`",
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
