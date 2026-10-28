import assert from "node:assert/strict"
import test from "node:test"
import {
  buildGeneratedMatrix,
  buildOxlintTestMatrix,
  buildPerfMatrix,
  buildReleaseMatrix,
  buildTypeScriptTestMatrix
} from "../src/matrix.ts"
import type { Upstream } from "../src/upstream.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"

test("builds a deterministic matrix for every unique TypeScript component", () => {
  const upstream: typeof Upstream.Type = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", beta: "7.1.0-beta", next: "7.1.0-dev" },
      "oxlint-tsgolint": { latest: "unused" },
      oxlint: { latest: "unused" }
    },
    components: {
      typescript: {
        "7.1.0-dev": { gitHead: revision, provider: "typescript" },
        "7.1.0-beta": { gitHead: revision, provider: "typescript" },
        "6.9.0": { gitHead: revision, provider: "typescript-go" },
        "7.0.2": { gitHead: revision, provider: "typescript-go" }
      },
      "oxlint-tsgolint": {},
      oxlint: {}
    },
    profiles: []
  }

  assert.deepEqual(buildTypeScriptTestMatrix(upstream), {
    include: [
      { name: "6.9.0", component: "typescript", version: "6.9.0", repoctl: false },
      { name: "latest", component: "typescript", version: "7.0.2", repoctl: false },
      { name: "beta", component: "typescript", version: "7.1.0-beta", repoctl: false },
      { name: "next", component: "typescript", version: "7.1.0-dev", repoctl: true }
    ]
  })
})

test("combines channel labels when latest, beta and next resolve to the same component", () => {
  const upstream: typeof Upstream.Type = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", beta: "7.0.2", next: "7.0.2" },
      "oxlint-tsgolint": { latest: "unused" },
      oxlint: { latest: "unused" }
    },
    components: {
      typescript: { "7.0.2": { gitHead: revision, provider: "typescript-go" } },
      "oxlint-tsgolint": {},
      oxlint: {}
    },
    profiles: []
  }

  assert.deepEqual(buildTypeScriptTestMatrix(upstream), {
    include: [{ name: "latest+beta+next", component: "typescript", version: "7.0.2", repoctl: true }]
  })
})

test("builds a component-aware generated branch matrix", () => {
  const upstream: typeof Upstream.Type = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", beta: "7.1.0-dev", next: "7.1.0-dev" },
      "oxlint-tsgolint": { latest: "unused" },
      oxlint: { latest: "unused" }
    },
    components: {
      typescript: {
        "7.0.2": { gitHead: revision, provider: "typescript-go" },
        "7.1.0-dev": { gitHead: revision, provider: "typescript" }
      },
      "oxlint-tsgolint": {},
      oxlint: {}
    },
    profiles: []
  }

  assert.deepEqual(buildGeneratedMatrix(upstream), {
    include: [{ name: "latest", component: "typescript", version: "7.0.2", branch: "generated/latest" }]
  })
})

test("builds a deduplicated matrix of compatible Oxlint component pairs", () => {
  const upstream: typeof Upstream.Type = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", beta: "7.1.0-dev", next: "7.1.0-dev" },
      "oxlint-tsgolint": { latest: "7.0.2001" },
      oxlint: { latest: "1.77.0" }
    },
    components: {
      typescript: {
        "7.0.2": { gitHead: revision, provider: "typescript-go" },
        "7.1.0-dev": { gitHead: revision, provider: "typescript" }
      },
      "oxlint-tsgolint": {
        "7.0.2001": { gitHead: revision, dependencies: { typescript: "7.0.2" } }
      },
      oxlint: {
        "1.76.0": { gitHead: revision },
        "1.77.0": { gitHead: revision }
      }
    },
    profiles: [
      {
        name: "vite-plus",
        description: "Vite+",
        dependencies: { oxlint: "1.76.0", "oxlint-tsgolint": "7.0.2001" }
      },
      {
        name: "vite-plus-alias",
        description: "Vite+ alias",
        dependencies: { oxlint: "1.76.0", "oxlint-tsgolint": "7.0.2001" }
      }
    ]
  }

  assert.deepEqual(buildOxlintTestMatrix(upstream), {
    include: [
      {
        name: "oxlint (1.76.0) + oxlint-tsgolint (7.0.2001)",
        oxlint: { component: "oxlint", version: "1.76.0" },
        tsgolint: { component: "oxlint-tsgolint", version: "7.0.2001" }
      },
      {
        name: "oxlint (1.77.0) + oxlint-tsgolint (7.0.2001)",
        oxlint: { component: "oxlint", version: "1.77.0" },
        tsgolint: { component: "oxlint-tsgolint", version: "7.0.2001" }
      }
    ]
  })

  const release = buildReleaseMatrix(upstream)
  assert.equal(release.include.length, 32)
  assert.deepEqual(release.include[0], {
    component: "typescript",
    version: "7.0.2",
    target: "darwin-arm64",
    runner: "ubuntu-latest",
    artifactName: "darwin-arm64__typescript__7.0.2",
    fileName: "tsc",
    destination: "_packages/tsgo-darwin-arm64/artifacts/typescript/7.0.2/tsc"
,
    timeoutMinutes: 45
  })
  assert.deepEqual(release.include.at(-1), {
    component: "oxlint",
    version: "1.77.0",
    target: "linux-arm64",
    runner: "ubuntu-latest",
    artifactName: "linux-arm64__oxlint__1.77.0",
    fileName: "oxlint.linux-arm64-gnu.node",
    destination: "_packages/tsgo-linux-arm64/artifacts/oxlint/1.77.0/oxlint.linux-arm64-gnu.node"
,
    timeoutMinutes: 45
  })
})

test("builds the tsrs binary once per tsrs version and tsrs platform, natively", () => {
  const versions = ["0.18.0-ts7.1.0-dev.20260929", "0.19.0-ts7.1.0-dev.20261020"]
  const upstream: typeof Upstream.Type = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", next: "7.1.0-dev" },
      "oxlint-tsgolint": { latest: "unused" },
      oxlint: { latest: "unused" },
      tsrs: { latest: versions[1]! }
    },
    components: {
      typescript: {
        "7.0.2": { gitHead: revision, provider: "typescript-go" },
        "7.1.0-dev": { gitHead: revision, provider: "typescript" }
      },
      "oxlint-tsgolint": {},
      oxlint: {},
      tsrs: Object.fromEntries(versions.map((version) => [
        version,
        { gitHead: revision, typescript: { version: "7.1.0-dev.20260929", gitHead: revision } }
      ]))
    },
    profiles: []
  }

  const release = buildReleaseMatrix(upstream).include
  // 2 TypeScript versions x 7 targets, independent of tsrs, then 2 tsrs versions x 5 targets.
  assert.equal(release.length, 24)
  assert.ok(release.filter((artifact) => artifact.component === "typescript").every((artifact) =>
    artifact.runner === "ubuntu-latest"))
  const tsrs = release.filter((artifact) => artifact.component === "tsrs")
  assert.deepEqual(
    tsrs.map(({ optimize, runner, target, timeoutMinutes, version }) =>
      `${version} ${target} ${runner} ${optimize} ${timeoutMinutes}`),
    versions.flatMap((version) => [
      `${version} darwin-arm64 macos-latest pgo 120`,
      `${version} darwin-x64 macos-26-intel pgo 120`,
      `${version} linux-x64 ubuntu-22.04 pgo-bolt 120`,
      `${version} linux-arm64 ubuntu-22.04-arm pgo-bolt 120`,
      `${version} win32-x64 windows-latest pgo 180`
    ])
  )
  assert.ok(release.filter((artifact) => artifact.component !== "tsrs").every((artifact) =>
    artifact.optimize === undefined && artifact.timeoutMinutes === 45))
  assert.equal(tsrs[4]?.fileName, "tsrs.exe")
  assert.deepEqual(tsrs[2], {
    component: "tsrs",
    version: versions[0],
    target: "linux-x64",
    runner: "ubuntu-22.04",
    artifactName: `linux-x64__tsrs__${versions[0]}`,
    fileName: "tsrs",
    destination: `_packages/tsgo-linux-x64/artifacts/tsrs/${versions[0]}/tsrs`,
    timeoutMinutes: 120,
    optimize: "pgo-bolt"
  })

  const { tsrs: _tags, ...tags } = upstream.tags
  const { tsrs: _components, ...components } = upstream.components
  assert.equal(
    buildReleaseMatrix({ ...upstream, tags, components }).include.some((artifact) => artifact.component === "tsrs"),
    false
  )
})

const typeScriptUpstream = (
  tags: { readonly latest: string; readonly beta: string; readonly next: string },
  versions: Record<string, "typescript" | "typescript-go">
): typeof Upstream.Type => ({
  schemaVersion: 5,
  tags: {
    typescript: tags,
    "oxlint-tsgolint": { latest: "unused" },
    oxlint: { latest: "unused" }
  },
  components: {
    typescript: Object.fromEntries(
      Object.entries(versions).map(([version, provider]) => [version, { gitHead: revision, provider }])
    ),
    "oxlint-tsgolint": {},
    oxlint: {}
  },
  profiles: []
})

const baseRef = "1111111111111111111111111111111111111111"
const headRef = "2222222222222222222222222222222222222222"

test("builds perf build entries per channel and side and measure entries per channel", () => {
  const base = typeScriptUpstream(
    { latest: "7.0.2", beta: "7.1.0-dev.20261007.1", next: "7.1.0-dev.20261007.1" },
    { "7.0.2": "typescript-go", "7.1.0-dev.20261007.1": "typescript" }
  )
  const head = typeScriptUpstream(
    { latest: "7.0.2", beta: "7.1.0-dev.20261014.1", next: "7.1.0-dev.20261014.1" },
    { "7.0.2": "typescript-go", "7.1.0-dev.20261014.1": "typescript" }
  )

  assert.deepEqual(buildPerfMatrix(base, head, baseRef, headRef), {
    build: {
      include: [
        { name: "latest", side: "base", ref: baseRef, version: "7.0.2" },
        { name: "beta+next", side: "base", ref: baseRef, version: "7.1.0-dev.20261007.1" },
        { name: "latest", side: "head", ref: headRef, version: "7.0.2" },
        { name: "beta+next", side: "head", ref: headRef, version: "7.1.0-dev.20261014.1" }
      ]
    },
    measure: {
      include: [
        {
          name: "latest",
          base: { ref: baseRef, version: "7.0.2" },
          head: { ref: headRef, version: "7.0.2" }
        },
        {
          name: "beta+next",
          base: { ref: baseRef, version: "7.1.0-dev.20261007.1" },
          head: { ref: headRef, version: "7.1.0-dev.20261014.1" }
        }
      ]
    }
  })
})

test("builds a perf matrix where a channel exists on only one side", () => {
  const base = typeScriptUpstream({ latest: "7.0.2", beta: "7.0.2", next: "7.0.2" }, { "7.0.2": "typescript-go" })
  const head = typeScriptUpstream(
    { latest: "7.0.2", beta: "7.1.0-dev.20261014.1", next: "7.1.0-dev.20261014.1" },
    { "7.0.2": "typescript-go", "7.1.0-dev.20261014.1": "typescript" }
  )

  assert.deepEqual(buildPerfMatrix(base, head, baseRef, headRef), {
    build: {
      include: [
        { name: "latest+beta+next", side: "base", ref: baseRef, version: "7.0.2" },
        { name: "latest", side: "head", ref: headRef, version: "7.0.2" },
        { name: "beta+next", side: "head", ref: headRef, version: "7.1.0-dev.20261014.1" }
      ]
    },
    measure: {
      include: [
        { name: "latest+beta+next", base: { ref: baseRef, version: "7.0.2" } },
        { name: "latest", head: { ref: headRef, version: "7.0.2" } },
        { name: "beta+next", head: { ref: headRef, version: "7.1.0-dev.20261014.1" } }
      ]
    }
  })
})

test("tests and releases the TypeScript the pinned tsrs ports like any other, but does not measure it", () => {
  const tsrsVersion = "0.18.0-ts7.1.0-dev.20260929"
  const upstream: typeof Upstream.Type = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", next: "7.1.0-dev.20261023.1" },
      "oxlint-tsgolint": { latest: "7.0.2005" },
      oxlint: { latest: "unused" },
      tsrs: { latest: tsrsVersion }
    },
    components: {
      typescript: {
        "7.0.2": { gitHead: revision, provider: "typescript-go" },
        "7.1.0-dev.20260929": { gitHead: revision, provider: "typescript" },
        "7.1.0-dev.20261023.1": { gitHead: revision, provider: "typescript" }
      },
      "oxlint-tsgolint": { "7.0.2005": { gitHead: revision, dependencies: { typescript: "7.0.2" } } },
      oxlint: {},
      tsrs: { [tsrsVersion]: { gitHead: revision, typescript: { version: "7.1.0-dev.20260929", gitHead: revision } } }
    },
    profiles: []
  }

  // Without a channel, its job is named after its version.
  assert.deepEqual(buildTypeScriptTestMatrix(upstream).include, [
    { name: "latest", component: "typescript", version: "7.0.2", repoctl: false },
    { name: "7.1.0-dev.20260929", component: "typescript", version: "7.1.0-dev.20260929", repoctl: false },
    { name: "next", component: "typescript", version: "7.1.0-dev.20261023.1", repoctl: true }
  ])
  const release = buildReleaseMatrix(upstream).include
  assert.deepEqual(
    [...new Set(release.filter((artifact) => artifact.component === "typescript").map((artifact) => artifact.version))],
    ["7.0.2", "7.1.0-dev.20260929", "7.1.0-dev.20261023.1"]
  )
  // No npm release has its commit, so perf has no stock compiler for it.
  assert.deepEqual(
    buildPerfMatrix(upstream, upstream, baseRef, headRef).build.include.map(({ name, side }) => `${side} ${name}`),
    ["base latest", "base next", "head latest", "head next"]
  )
  assert.deepEqual(
    buildPerfMatrix(upstream, upstream, baseRef, headRef).measure.include.map(({ name }) => name),
    ["latest", "next"]
  )
})
