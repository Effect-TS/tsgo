import { describe, expect, it } from "vitest"
import {
  getPatchCommand,
  getPatchIntegrations,
  hasPatchCommand,
  updatePatchCommand,
  vitePlusLintsWithTsrs
} from "../../src/cli/setup/patch-command.js"

describe("setup patch command", () => {
  it.each([
    [["typescript"] as const, "effect-tsgo patch --typescript --no-oxlint"],
    [["oxlint"] as const, "effect-tsgo patch --no-typescript --oxlint"],
    [["typescript", "oxlint"] as const, "effect-tsgo patch --typescript --oxlint"]
  ])("updates integration flags for %j", (integrations, expected) => {
    expect(updatePatchCommand("effect-tsgo patch --oxlint", integrations)).toEqual({
      script: expected,
      found: true
    })
  })

  it("preserves wrappers, unrelated commands, and unrelated flags", () => {
    expect(updatePatchCommand(
      "husky && pnpm exec effect-tsgo patch --oxlint --force && echo ready",
      ["typescript"]
    )).toEqual({
      script: "husky && pnpm exec effect-tsgo patch --typescript --no-oxlint --force && echo ready",
      found: true
    })
  })

  it("keeps the experimental --tsrs integration flag when updating integrations", () => {
    expect(updatePatchCommand("effect-tsgo patch --tsrs --typescript --no-oxlint", ["typescript", "oxlint"])).toEqual({
      script: "effect-tsgo patch --typescript --oxlint --tsrs",
      found: true
    })
    expect(getPatchIntegrations("effect-tsgo patch --tsrs")).toEqual(["typescript"])
  })

  it("adds --tsrs for the tsrs integration, once", () => {
    expect(getPatchCommand(["typescript", "oxlint", "tsrs"])).toBe("effect-tsgo patch --typescript --oxlint --tsrs")
    expect(updatePatchCommand("effect-tsgo patch --typescript --oxlint", ["typescript", "oxlint", "tsrs"])).toEqual({
      script: "effect-tsgo patch --typescript --oxlint --tsrs",
      found: true
    })
    expect(updatePatchCommand("effect-tsgo patch --tsrs --typescript --oxlint", ["typescript", "oxlint", "tsrs"])).toEqual({
      script: "effect-tsgo patch --typescript --oxlint --tsrs",
      found: true
    })
  })

  it("detects a vite-plus that lints with tsrs from its version spec", () => {
    for (const spec of ["1.7.0", "^1.7.0", "~1.8.2", ">=1.7.0", "2.0.0-beta.1"]) expect(vitePlusLintsWithTsrs(spec)).toBe(true)
    for (const spec of ["1.6.0", "^1.5.1", "latest", "catalog:", "workspace:*"]) expect(vitePlusLintsWithTsrs(spec)).toBe(false)
  })

  it("removes only the patch invocation from a command list", () => {
    expect(updatePatchCommand("husky && effect-tsgo patch --typescript --oxlint", [])).toEqual({
      script: "husky",
      found: true
    })
  })

  it("preserves command order when removing from the middle of a list", () => {
    expect(updatePatchCommand("lint; effect-tsgo patch --oxlint && build", [])).toEqual({
      script: "lint; build",
      found: true
    })
  })

  it("refuses to remove commands from ambiguous control flow", () => {
    const script = "lint && effect-tsgo patch --oxlint || recover"
    expect(updatePatchCommand(script, [])).toEqual({ script, found: false })
  })

  it("does not treat command arguments as an invocation", () => {
    expect(hasPatchCommand("echo effect-tsgo patch && echo done")).toBe(false)
  })

  it("detects explicit and default integrations", () => {
    expect(getPatchIntegrations("effect-tsgo patch")).toEqual(["typescript"])
    expect(getPatchIntegrations("effect-tsgo patch --no-typescript --oxlint")).toEqual(["oxlint"])
  })
})
