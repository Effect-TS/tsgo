import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import { describe, expect, it } from "vitest"
import { assess } from "../../src/cli/setup/assessment.js"
import { computeChanges } from "../../src/cli/setup/changes.js"
import * as Target from "../../src/cli/setup/target.js"

const context: Target.Context = {
  defaultLspVersion: "0.71.0",
  defaultTypescriptVersion: "7.0.2",
  defaultOxlintVersion: "1.94.0",
  defaultOxlintTsgolintVersion: "7.0.2007",
  defaultSchemaPath: "/test/node_modules/@effect/tsgo/schema.json",
  defaultOxlintrcSchemaPath: "/test/node_modules/@effect/tsgo/oxlint-schema.json"
}

const setUp = (dependencies: Record<string, string>, integrations: ReadonlyArray<"typescript" | "oxlint">) => {
  const packageJsonText = JSON.stringify({ name: "test-project", devDependencies: dependencies }, null, 2)
  const assessment = assess({
    packageJson: { fileName: "/test/package.json", text: packageJsonText },
    tsconfig: { fileName: "/test/tsconfig.json", text: "{}" },
    oxlintConfig: Option.none(),
    vscodeSettings: Option.none()
  })
  const target = Effect.runSync(
    Target.create(assessment, context, {
      integrations,
      dependencyType: "devDependencies",
      diagnosticSeverities: {},
      editors: []
    }).pipe(Effect.provide(NodeServices.layer))
  )
  const change = computeChanges(assessment, target).codeActions
    .flatMap((action) => action.changes)
    .find((item) => item.fileName === "/test/package.json")
  const text = [...change?.textChanges ?? []]
    .sort((left, right) => right.span.start - left.span.start)
    .reduce(
      (result, item) => result.slice(0, item.span.start) + item.newText + result.slice(item.span.start + item.span.length),
      packageJsonText
    )
  return { target, packageJson: JSON.parse(text) as { scripts?: { prepare?: string }; devDependencies: Record<string, string> } }
}

describe("setup target", () => {
  // vite-plus 1.7.0 lints with its tsrs, so the Oxlint integration patches that tsrs too (Effect-TS/tsgo#1062).
  it("patches the tsrs of a vite-plus that lints with tsrs", () => {
    const { packageJson, target } = setUp({ "vite-plus": "^1.7.0" }, ["typescript", "oxlint"])
    expect(target.packageJson.integrations).toEqual(["typescript", "oxlint", "tsrs"])
    expect(packageJson.scripts?.prepare).toBe("effect-tsgo patch --typescript --oxlint --tsrs")
    expect(packageJson.devDependencies.oxlint).toBeUndefined()
    expect(packageJson.devDependencies["oxlint-tsgolint"]).toBeUndefined()
  })

  it("patches it next to an oxlint of the project's own, which vite-plus still lints with tsrs", () => {
    expect(setUp({ "vite-plus": "^1.7.0", oxlint: "1.94.0" }, ["typescript", "oxlint"]).packageJson.scripts?.prepare)
      .toBe("effect-tsgo patch --typescript --oxlint --tsrs")
  })

  it("leaves tsrs alone for an older vite-plus, a plain Oxlint and the TypeScript integration alone", () => {
    expect(setUp({ "vite-plus": "^1.6.0" }, ["typescript", "oxlint"]).packageJson.scripts?.prepare)
      .toBe("effect-tsgo patch --typescript --oxlint")
    expect(setUp({ oxlint: "1.94.0" }, ["typescript", "oxlint"]).packageJson.scripts?.prepare)
      .toBe("effect-tsgo patch --typescript --oxlint")
    expect(setUp({ "vite-plus": "^1.7.0" }, ["typescript"]).packageJson.scripts?.prepare)
      .toBe("effect-tsgo patch --typescript --no-oxlint")
  })
})
