import { describe, expect, it } from "vitest"
import { getPatchIntegrations, hasPatchCommand, updatePatchCommand } from "../../src/cli/setup/patch-command.js"

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
