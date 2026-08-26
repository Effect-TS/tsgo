import assert from "node:assert/strict"
import test from "node:test"
import {
  buildGeneratedMatrix,
  buildOxlintTestMatrix,
  buildReleaseMatrix,
  buildTypeScriptTestMatrix
} from "../src/matrix.ts"
import type { Upstream } from "../src/upstream.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"

test("builds a deterministic matrix for every unique TypeScript component", () => {
  const upstream: typeof Upstream.Type = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", next: "7.1.0-dev" },
      "oxlint-tsgolint": { latest: "unused" },
      oxlint: { latest: "unused" }
    },
    components: {
      typescript: {
        "7.1.0-dev": { gitHead: revision, provider: "typescript" },
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
      { name: "next", component: "typescript", version: "7.1.0-dev", repoctl: true }
    ]
  })
})

test("combines channel labels when latest and next resolve to the same component", () => {
  const upstream: typeof Upstream.Type = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", next: "7.0.2" },
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
    include: [{ name: "latest+next", component: "typescript", version: "7.0.2", repoctl: true }]
  })
})

test("builds a component-aware generated branch matrix", () => {
  const upstream: typeof Upstream.Type = {
    schemaVersion: 5,
    tags: {
      typescript: { latest: "7.0.2", next: "7.1.0-dev" },
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
      typescript: { latest: "7.0.2", next: "7.1.0-dev" },
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
  })
  assert.deepEqual(release.include.at(-1), {
    component: "oxlint",
    version: "1.77.0",
    target: "linux-arm64",
    runner: "ubuntu-latest",
    artifactName: "linux-arm64__oxlint__1.77.0",
    fileName: "oxlint.linux-arm64-gnu.node",
    destination: "_packages/tsgo-linux-arm64/artifacts/oxlint/1.77.0/oxlint.linux-arm64-gnu.node"
  })
})
