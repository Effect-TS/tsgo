import assert from "node:assert/strict"
import test from "node:test"
import { buildGeneratedMatrix, buildOxlintTestMatrix, buildTypeScriptTestMatrix } from "../src/matrix.ts"
import type { Upstream } from "../src/upstream.ts"

const revision = "0123456789abcdef0123456789abcdef01234567"

test("builds a deterministic matrix for every unique TypeScript component", () => {
  const upstream: typeof Upstream.Type = {
    schemaVersion: 3,
    typescript: { latest: "7.0.2", next: "7.1.0-dev" },
    components: {
      typescript: {
        "7.1.0-dev": { gitHead: revision },
        "6.9.0": { gitHead: revision },
        "7.0.2": { gitHead: revision }
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
    schemaVersion: 3,
    typescript: { latest: "7.0.2", next: "7.0.2" },
    components: {
      typescript: { "7.0.2": { gitHead: revision } },
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
    schemaVersion: 3,
    typescript: { latest: "7.0.2", next: "7.1.0-dev" },
    components: {
      typescript: {
        "7.0.2": { gitHead: revision },
        "7.1.0-dev": { gitHead: revision }
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
    schemaVersion: 3,
    typescript: { latest: "7.0.2", next: "7.1.0-dev" },
    components: {
      typescript: {
        "7.0.2": { gitHead: revision },
        "7.1.0-dev": { gitHead: revision }
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
        name: "oxlint",
        description: "Latest Oxlint",
        dependencies: { oxlint: "1.77.0", "oxlint-tsgolint": "7.0.2001" }
      },
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
        name: "vite-plus+vite-plus-alias",
        oxlint: { component: "oxlint", version: "1.76.0" },
        tsgolint: { component: "oxlint-tsgolint", version: "7.0.2001" }
      },
      {
        name: "oxlint",
        oxlint: { component: "oxlint", version: "1.77.0" },
        tsgolint: { component: "oxlint-tsgolint", version: "7.0.2001" }
      }
    ]
  })
})
