import { describe, expect, it } from "vitest"
import {
  applyPresetDiagnosticSeverities,
  compareRuleSeverity,
  isPresetEnabled,
  mergePresetDiagnosticSeverities,
  presets
} from "../src/cli/presets.js"

describe("diagnostic presets", () => {
  it("exposes the complete preset catalog", () => {
    expect(presets.map((preset) => preset.name)).toEqual([
      "recommended",
      "strict",
      "correctness",
      "antipattern",
      "effect-native",
      "style"
    ])
  })

  it("makes every diagnostic enabled by default an error in strict mode", () => {
    const strict = presets.find((preset) => preset.name === "strict")!

    expect(strict.diagnosticSeverity).not.toEqual({})
    expect(Object.values(strict.diagnosticSeverity).every((severity) => severity === "error")).toBe(true)
  })

  it("uses only default-enabled diagnostics in the recommended preset", () => {
    const recommended = presets.find((preset) => preset.name === "recommended")!

    expect(recommended.diagnosticSeverity.floatingEffect).toBe("error")
    expect(recommended.diagnosticSeverity.globalDate).toBeUndefined()
  })

  it("merges the selected preset severities", () => {
    expect(mergePresetDiagnosticSeverities(["effect-native"])).toEqual(
      presets.find((preset) => preset.name === "effect-native")!.diagnosticSeverity
    )
  })

  it("applies preset severities as minimums on top of existing config", () => {
    const merged = applyPresetDiagnosticSeverities(
      {
        globalFetch: "error"
      },
      ["effect-native"]
    )

    expect(merged.globalFetch).toBe("error")
    expect(merged.extendsNativeError).toBe("warning")
    expect(merged.preferSchemaOverJson).toBe("warning")
  })

  it("normalizes existing diagnostic keys before applying presets", () => {
    const merged = applyPresetDiagnosticSeverities(
      {
        globalfetch: "error"
      },
      ["effect-native"]
    )

    expect(merged.globalFetch).toBe("error")
    expect(merged.globalfetch).toBeUndefined()
  })

  it("computes enablement using effective severities", () => {
    const merged = applyPresetDiagnosticSeverities({}, ["effect-native"])

    expect(isPresetEnabled("effect-native", merged)).toBe(true)
    expect(compareRuleSeverity(merged.globalFetch!, "warning")).toBeGreaterThanOrEqual(0)
  })
})
